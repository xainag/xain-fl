use console::strip_ansi_codes;
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::{
    api::core::v1::{ConfigMap, Pod},
    apimachinery::pkg::apis::meta::v1::Time,
};

use kube::{
    api::{Api, DeleteParams, ListParams, LogParams, Meta, PatchParams, PatchStrategy, WatchEvent},
    Client,
};
use serde_json::json;
use std::{path::Path, process::Stdio};
use tokio::{
    fs,
    fs::File,
    io::AsyncWriteExt,
    process::{Child, Command},
    task::JoinHandle,
};

use super::K8sSettings;

#[derive(Clone)]
pub struct K8sClient {
    settings: K8sSettings,
    pod_api: Api<Pod>,
    config_map_api: Api<ConfigMap>,
}

impl K8sClient {
    pub async fn new(settings: K8sSettings) -> anyhow::Result<Self> {
        let client = Client::try_default().await?;

        let pod_api: Api<Pod> = Api::namespaced(client.clone(), &settings.namespace);
        let config_map_api: Api<ConfigMap> = Api::namespaced(client, &settings.namespace);

        Ok(Self {
            settings,
            pod_api,
            config_map_api,
        })
    }

    pub async fn deploy_with_config(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let pod = self.find_pod(&self.settings.coordinator_pod_label).await?;
        let (pod_name, config_map_name) = Self::reveal_pod_and_config_map_name(&pod)?;
        let config_content = fs::read_to_string(path).await?;
        self.patch_config_map(&config_map_name, config_content)
            .await?;
        self.restart_pod(&pod_name, &self.settings.coordinator_pod_label)
            .await?;
        Ok(())
    }

    pub async fn deploy_with_image(&self) -> anyhow::Result<()> {
        let pod = self.find_pod(&self.settings.coordinator_pod_label).await?;
        self.patch_image(
            &PodSpec::name(&pod),
            &self.settings.coordinator_pod_label,
            &self.settings.coordinator_image,
        )
        .await?;
        Ok(())
    }

    pub async fn deploy_with_image_and_config(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let pod = self.find_pod(&self.settings.coordinator_pod_label).await?;
        let (pod_name, config_map_name) = Self::reveal_pod_and_config_map_name(&pod)?;
        let config_content = fs::read_to_string(path).await?;
        self.patch_config_map(&config_map_name, config_content)
            .await?;
        self.patch_image(
            &pod_name,
            &self.settings.coordinator_pod_label,
            &self.settings.coordinator_image,
        )
        .await?;
        Ok(())
    }

    pub async fn restart_coordinator(&self) -> anyhow::Result<()> {
        let pod = self.find_pod(&self.settings.coordinator_pod_label).await?;
        self.restart_pod(&PodSpec::name(&pod), &self.settings.coordinator_pod_label)
            .await?;
        Ok(())
    }

    pub async fn kill_influxdb(&self) -> anyhow::Result<()> {
        self.delete_pod(&self.settings.influxdb_pod_name).await
    }

    pub async fn kill_redis(&self) -> anyhow::Result<()> {
        self.delete_pod(&self.settings.redis_pod_name).await
    }

    pub async fn kill_s3(&self) -> anyhow::Result<()> {
        let pod = self.find_pod(&self.settings.s3_pod_label).await?;
        self.delete_pod(&PodSpec::name(&pod)).await
    }

    pub async fn port_forward_coordinator(&self) -> anyhow::Result<PortForward> {
        let pod = self.find_pod(&self.settings.coordinator_pod_label).await?;
        let process = Self::new_port_forward(&PodSpec::name(&pod), "8081:8081")?;
        Ok(PortForward { process })
    }

    pub fn port_forward_influxdb(&self) -> anyhow::Result<PortForward> {
        let process = Self::new_port_forward(&self.settings.influxdb_pod_name, "8086:8086")?;
        Ok(PortForward { process })
    }

    pub fn port_forward_redis(&self) -> anyhow::Result<PortForward> {
        let process = Self::new_port_forward(&self.settings.redis_pod_name, "6379:6379")?;
        Ok(PortForward { process })
    }

    pub async fn port_forward_s3(&self) -> anyhow::Result<PortForward> {
        let pod = self.find_pod(&self.settings.s3_pod_label).await?;
        let process = Self::new_port_forward(&PodSpec::name(&pod), "9000:9000")?;
        Ok(PortForward { process })
    }

    pub async fn save_coordinator_logs(
        &self,
        path: &str,
    ) -> anyhow::Result<JoinHandle<anyhow::Result<()>>> {
        let pod = self.find_pod(&self.settings.coordinator_pod_label).await?;
        info!("writing {} log into: {}", PodSpec::name(&pod), path);

        let lp = LogParams {
            follow: true,
            ..Default::default()
        };
        let mut logs = self
            .pod_api
            .log_stream(&PodSpec::name(&pod), &lp)
            .await?
            .boxed();
        let path = path.to_string();

        let handle = tokio::spawn(async move {
            let mut file = File::create(path).await?;
            while let Some(line) = logs.try_next().await? {
                let log = &String::from_utf8_lossy(&line);
                file.write_all(strip_ansi_codes(log).as_bytes()).await?;
            }
            Ok::<(), anyhow::Error>(())
        });

        Ok(handle)
    }
}

impl K8sClient {
    async fn find_pod(&self, label: &str) -> anyhow::Result<Pod> {
        info!("searching for pod with label: {}", label);

        let lp = ListParams::default().labels(label);
        let pods = self.pod_api.list(&lp).await?;

        let mut pods = pods.items;
        pods.sort_by(|a, b| PodSpec::start_time(&b).cmp(&PodSpec::start_time(&a)));
        let found_pod = pods
            .into_iter()
            .find(PodSpec::is_running)
            .ok_or_else(|| anyhow::anyhow!("cannot find pod with label: {}", label))?;

        Ok(found_pod)
    }

    async fn patch_config_map(
        &self,
        config_map_name: &str,
        config_content: String,
    ) -> anyhow::Result<()> {
        info!("patching config map: {}", config_map_name);

        let config_patch = json!(
            {
                "data": {
                    "config.toml":  config_content
                }
            }
        );

        let pp = PatchParams {
            patch_strategy: PatchStrategy::Strategic,
            ..Default::default()
        };
        self.config_map_api
            .patch(config_map_name, &pp, serde_json::to_vec(&config_patch)?)
            .await?;
        info!("patched config map: {}", config_map_name);
        Ok(())
    }

    async fn restart_pod(&self, pod_name: &str, label: &str) -> anyhow::Result<String> {
        info!("restarting pod: {}", pod_name);
        self.delete_pod(pod_name).await?;

        let new_pod_name = self.wait_until_restarted(label).await?;

        info!("new pod id: {}", new_pod_name);
        Ok(new_pod_name)
    }

    async fn delete_pod(&self, pod_name: &str) -> anyhow::Result<()> {
        let dp = DeleteParams::default();
        self.pod_api.delete(pod_name, &dp).await?.map_left(|pdel| {
            info!("deleting pod: {}", PodSpec::name(&pdel));
        });

        Ok(())
    }

    async fn wait_until_restarted(&self, label: &str) -> anyhow::Result<String> {
        let lp = ListParams::default().labels(label);
        let mut stream = self.pod_api.watch(&lp, "0").await?.boxed();

        loop {
            if let Some(status) = stream.try_next().await? {
                match status {
                    WatchEvent::Added(o) => info!("added {}", PodSpec::name(&o)),
                    WatchEvent::Modified(o) => {
                        let s = o.status.as_ref().expect("status exists on pod");
                        let phase = s.phase.clone().unwrap_or_default();
                        info!("modified: {} with phase: {}", PodSpec::name(&o), phase);
                        if phase == "Running" {
                            break Ok(PodSpec::name(&o));
                        }
                    }
                    WatchEvent::Deleted(o) => info!("deleted {}", Meta::name(&o)),
                    WatchEvent::Error(e) => error!("error {}", e),
                    _ => {}
                }
            }
        }
    }

    async fn patch_image(
        &self,
        pod_name: &str,
        label: &str,
        image: &str,
    ) -> anyhow::Result<String> {
        info!("patching image of pod: {}", pod_name);

        let pp = PatchParams {
            patch_strategy: PatchStrategy::Strategic,
            ..Default::default()
        };
        let image_patch = json!(
            {
                "spec": {
                    "containers": [
                        {
                            "name": "coordinator",
                            "image": image
                        }
                    ]
                }
            }
        );

        self.pod_api
            .patch(pod_name, &pp, serde_json::to_vec(&image_patch)?)
            .await?;

        let new_pod_name = self.wait_until_restarted(label).await?;

        info!("Patched pod: {}", new_pod_name);
        Ok(new_pod_name)
    }

    fn reveal_pod_and_config_map_name(pod: &Pod) -> anyhow::Result<(String, String)> {
        let pod_name = PodSpec::name(pod);
        let config_map_name = PodSpec::config_map_name(pod)
            .ok_or_else(|| anyhow::anyhow!("cannot find config map name"))?;
        info!(
            "pod name: {}, config map name: {}",
            pod_name, config_map_name
        );
        Ok((pod_name, config_map_name))
    }

    fn new_port_forward(pod_name: &str, port_mapping: &str) -> anyhow::Result<Child> {
        info!(
            "new port forward for pod: {} with port mapping: {}",
            pod_name, port_mapping
        );
        let handle = Command::new("kubectl")
            .arg("port-forward")
            .arg(pod_name)
            .arg(port_mapping)
            .stdout(Stdio::null())
            .spawn()?;
        Ok(handle)
    }
}

pub struct PortForward {
    process: Child,
}

impl Drop for PortForward {
    fn drop(&mut self) {
        info!("close port forward");
        self.process.kill().unwrap();
    }
}

struct PodSpec;
impl PodSpec {
    fn config_map_name(pod: &Pod) -> Option<String> {
        pod.spec
            .as_ref()?
            .volumes
            .as_ref()?
            .get(1)?
            .config_map
            .as_ref()?
            .name
            .clone()
    }

    fn name(pod: &Pod) -> String {
        Meta::name(pod)
    }

    fn is_running(pod: &Pod) -> bool {
        let s = pod.status.as_ref().expect("status exists on pod");
        let phase = s.phase.clone().unwrap_or_default();
        phase == "Running"
    }

    fn start_time(pod: &Pod) -> Time {
        let s = pod.status.as_ref().expect("status exists on pod");
        s.start_time.as_ref().expect("start time").clone()
    }
}
