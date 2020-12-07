use super::{influx::InfluxClient, k8s::K8sClient, TestEnvironmentSettings};
use std::path::PathBuf;
use tracing_subscriber::{fmt::Formatter, reload::Handle, EnvFilter, FmtSubscriber};

use xaynet_sdk::client::Client as ApiClient;
use xaynet_server::{
    settings::Settings,
    storage::{coordinator_storage::redis, model_storage::s3},
};

#[allow(dead_code)]
pub struct TestEnvironment {
    k8s_client: Option<K8sClient>,
    api_client: Option<ApiClient<reqwest::Client>>,
    influx_client: Option<InfluxClient>,
    redis_client: Option<redis::Client>,
    s3_client: Option<s3::Client>,
    filter_handle: Handle<EnvFilter, Formatter>,
    settings: TestEnvironmentSettings,
}

impl TestEnvironment {
    pub async fn new(settings: TestEnvironmentSettings) -> anyhow::Result<Self> {
        let filter_handle = Self::init_tracing(&settings.filter);

        Ok(Self {
            k8s_client: None,
            api_client: None,
            influx_client: None,
            redis_client: None,
            s3_client: None,
            filter_handle,
            settings,
        })
    }

    fn init_tracing(filter: impl Into<EnvFilter>) -> Handle<EnvFilter, Formatter> {
        let fmt_subscriber = FmtSubscriber::builder()
            .with_env_filter(filter)
            .with_ansi(true)
            .with_filter_reloading();
        let filter_handle = fmt_subscriber.reload_handle();
        fmt_subscriber.init();
        filter_handle
    }

    #[allow(dead_code)]
    fn reload_filter(&self, filter: impl Into<EnvFilter>) {
        let _ = self.filter_handle.reload(filter);
    }

    async fn init_k8s_client(&mut self) -> anyhow::Result<()> {
        self.k8s_client = Some(K8sClient::new(self.settings.k8s.clone()).await?);
        Ok(())
    }

    fn init_api_client(&mut self) -> anyhow::Result<()> {
        // let certificates = self
        //     .settings
        //     .api_client
        //     .certificates
        //     .as_ref()
        //     .map(ApiClient::certificates_from)
        //     .transpose()?;

        // let identity = self
        //     .settings
        //     .api_client
        //     .identity
        //     .as_ref()
        //     .map(ApiClient::identity_from)
        //     .transpose()?;

        let http_client = reqwest::ClientBuilder::new().build().unwrap();
        let api_client = ApiClient::new(http_client, &self.settings.api_client.address)?;
        self.api_client = Some(api_client);
        Ok(())
    }

    fn init_influx_client(&mut self) {
        self.influx_client = Some(InfluxClient::new(self.settings.influx.clone()));
    }

    async fn init_redis_client(&mut self) -> anyhow::Result<()> {
        self.redis_client = Some(redis::Client::new(self.settings.redis.url.clone()).await?);
        Ok(())
    }

    pub fn init_s3_client(&mut self) -> anyhow::Result<()> {
        self.s3_client = Some(s3::Client::new(self.settings.s3.clone())?);
        Ok(())
    }

    pub async fn get_k8s_client(&mut self) -> anyhow::Result<K8sClient> {
        if self.k8s_client.is_none() {
            self.init_k8s_client().await?;
        }

        Ok(self.k8s_client.clone().unwrap())
    }

    pub fn get_api_client(&mut self) -> anyhow::Result<ApiClient<reqwest::Client>> {
        if self.api_client.is_none() {
            self.init_api_client()?;
        }

        Ok(self.api_client.clone().unwrap())
    }

    pub fn get_influx_client(&mut self) -> InfluxClient {
        if self.influx_client.is_none() {
            self.init_influx_client();
        }

        self.influx_client.clone().unwrap()
    }

    pub async fn get_redis_client(&mut self) -> anyhow::Result<redis::Client> {
        if self.redis_client.is_none() {
            self.init_redis_client().await?;
        }

        Ok(self.redis_client.clone().unwrap())
    }

    pub async fn get_s3_client(&mut self) -> anyhow::Result<s3::Client> {
        if self.s3_client.is_none() {
            self.init_s3_client()?;
        }

        Ok(self.s3_client.clone().unwrap())
    }

    pub fn get_env_settings(&self) -> TestEnvironmentSettings {
        self.settings.clone()
    }

    pub fn get_coordinator_settings(&self) -> anyhow::Result<Settings> {
        let settings = Settings::new(PathBuf::from(&self.settings.coordinator.config))?;
        Ok(settings)
    }
}
