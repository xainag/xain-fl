pub mod environment;
pub mod influx;
pub mod k8s;
pub mod utils;

pub use self::environment::TestEnvironment;

use config::Config;
use serde::Deserialize;
use std::path::PathBuf;
use xaynet_server::settings::{InfluxSettings, RedisSettings, S3Settings};

#[derive(Deserialize, Clone)]
pub struct TestEnvironmentSettings {
    pub filter: String,
    pub k8s: K8sSettings,
    pub coordinator: CoordinatorSettings,
    pub influx: InfluxSettings,
    pub redis: RedisSettings,
    pub s3: S3Settings,
    pub api_client: ApiClientSettings,
}

impl TestEnvironmentSettings {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let mut path = PathBuf::from(path);
        path.push("Env.toml");
        let settings: TestEnvironmentSettings = Self::load(path)?;
        Ok(settings)
    }

    fn load(path: PathBuf) -> anyhow::Result<Self> {
        let mut config = Config::new();
        config.merge(config::File::from(path))?;
        config
            .try_into()
            .map_err(|e| anyhow::anyhow!("config error: {}", e))
    }
}

#[derive(Deserialize, Clone)]
pub struct K8sSettings {
    pub namespace: String,
    pub coordinator_pod_label: String,
    pub coordinator_image: String,
    pub influxdb_pod_name: String,
    pub redis_pod_name: String,
    pub s3_pod_label: String,
}

#[derive(Deserialize, Clone)]
pub struct CoordinatorSettings {
    pub config: String,
}

#[derive(Deserialize, Clone)]
pub struct ApiClientSettings {
    pub address: String,
    pub certificates: Option<Vec<PathBuf>>,
    pub identity: Option<PathBuf>,
}
