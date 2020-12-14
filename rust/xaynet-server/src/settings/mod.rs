//! Loading and validation of settings.
//!
//! Values defined in the configuration file can be overridden by environment variables. Examples of
//! configuration files can be found in the `configs/` directory located in the repository root.

#[cfg(feature = "tls")]
use std::path::PathBuf;
use std::{fmt, path::Path};

use config::{Config, ConfigError, Environment};
use redis::{ConnectionInfo, IntoConnectionInfo};
use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize,
};
use thiserror::Error;
use tracing_subscriber::filter::EnvFilter;
use validator::{Validate, ValidationError, ValidationErrors};

use xaynet_core::{
    mask::{BoundType, DataType, GroupType, MaskConfig, ModelType},
    message::{MIN_SUM_COUNT, MIN_UPDATE_COUNT},
};

#[cfg(feature = "model-persistence")]
#[cfg_attr(docsrs, doc(cfg(feature = "model-persistence")))]
pub mod s3;
#[cfg(feature = "model-persistence")]
pub use self::{s3::RestoreSettings, s3::S3BucketsSettings, s3::S3Settings};

#[cfg(feature = "iota")]
pub mod iota;
#[cfg(feature = "iota")]
pub use self::iota::IotaSettings;

#[derive(Error, Debug)]
/// An error related to loading and validation of settings.
pub enum SettingsError {
    #[error("configuration loading failed: {0}")]
    Loading(#[from] ConfigError),
    #[error("validation failed: {0}")]
    Validation(#[from] ValidationErrors),
}

#[derive(Debug, Validate, Deserialize)]
/// The combined settings.
///
/// Each section in the configuration file corresponds to the identically named settings field.
pub struct Settings {
    pub api: ApiSettings,
    #[validate]
    pub pet: PetSettings,
    pub mask: MaskSettings,
    pub log: LoggingSettings,
    pub model: ModelSettings,
    #[validate]
    pub metrics: MetricsSettings,
    pub redis: RedisSettings,
    #[cfg(feature = "model-persistence")]
    #[validate]
    pub s3: S3Settings,
    #[cfg(feature = "model-persistence")]
    #[validate]
    pub restore: RestoreSettings,
    #[cfg_attr(not(feature = "iota"), serde(default))]
    pub trust_anchor: TrustAnchorSettings,
}

impl Settings {
    /// Loads and validates the settings via a configuration file.
    ///
    /// # Errors
    /// Fails when the loading of the configuration file or its validation failed.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let settings: Settings = Self::load(path)?;
        settings.validate()?;
        Ok(settings)
    }

    fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let mut config = Config::new();
        config.merge(config::File::from(path.as_ref()))?;
        config.merge(Environment::with_prefix("xaynet").separator("__"))?;
        config.try_into()
    }
}

#[derive(Debug, Validate, Deserialize, Clone, Copy)]
#[validate(schema(function = "validate_pet"))]
/// PET protocol settings.
pub struct PetSettings {
    /// The minimal number of participants selected for preparing the unmasking. The value must be
    /// greater or equal to `1` (i.e. `min_sum_count >= 1`) for the PET protocol to function
    /// correctly.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// min_sum_count = 10
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__MIN_SUM_COUNT=10
    /// ```
    pub min_sum_count: u64,

    /// The expected number of participants selected for submitting an updated local model for
    /// aggregation. The value must be greater or equal to `3` (i.e. `min_update_count >= 3`) for
    /// the PET protocol to function correctly.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// min_update_count = 100
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__MIN_UPDATE_COUNT=100
    /// ```
    pub min_update_count: u64,

    /// The minimal number of participants selected for submitting the aggregated masks. The value
    /// must be greater or equal to `1` (i.e. `min_sum2_count >= 1`) for the PET protocol to
    /// function correctly and less or equal to `max_sum_count` (i.e. `min_sum2_count <=
    /// max_sum_count`).
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// min_sum2_count = 10
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__MIN_SUM2_COUNT=10
    /// ```
    pub min_sum2_count: u64,

    /// The maximal number of participants selected for preparing the unmasking. No more messages
    /// will be processed in the `sum` phase if the `min_sum_time` has not yet elapsed. The value
    /// must be greater or equal to `min_sum_count` (i.e. `min_sum_count <= max_sum_count`).
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// max_sum_count = 100
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__MAX_SUM_COUNT=100
    /// ```
    pub max_sum_count: u64,

    /// The maximal number of participants selected for submitting an updated local model for
    /// aggregation. No more message will be processed in the `update` phase if the
    /// `min_update_time` has not yet elapsed. The value must be greater or equal to
    /// `min_update_count` (i.e. `min_update_count <= max_update_count`).
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// max_update_count = 10000
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__MAX_UPDATE_COUNT=10000
    /// ```
    pub max_update_count: u64,

    /// The maximal number of participants selected for submitting the aggregated masks. No more
    /// messages will be processed in the `sum2` phase if the `min_sum2_time` has not yet elapsed.
    /// The value must be greater or equal to `min_sum2_count` (i.e. `min_sum2_count <=
    /// max_sum2_count`) and less or equal to `max_sum_count` (i.e. `max_sum2_count <=
    /// max_sum_count`).
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// max_sum2_count = 100
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__MAX_SUM2_COUNT=100
    /// ```
    pub max_sum2_count: u64,

    /// The minimum amount of time reserved for processing messages in the `sum` phase, in seconds.
    /// Once this time has passed, the `sum` phase ends *as soon as* `min_sum_count` messages have
    /// been processed. Set this higher to allow for the possibility of more than `min_sum_count`
    /// messages to be processed in the `sum` phase.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// min_sum_time = 5
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__MIN_SUM_TIME=5
    /// ```
    pub min_sum_time: u64,

    /// The minimum amount of time reserved for processing messages in the `update` phase, in
    /// seconds. Once this time has passed, the `update` phase ends *as soon as* `min_update_count`
    /// messages have been processed. Set this higher to allow for the possibility of more than
    /// `min_update_count` messages to be processed in the `update` phase.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// min_update_time = 10
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__MIN_UPDATE_TIME=10
    /// ```
    pub min_update_time: u64,

    /// The minimum amount of time reserved for processing messages in the `sum2` phase, in seconds.
    /// Once this time has passed, the `sum2` phase ends *as soon as* `min_sum2_count` messages have
    /// been processed. Set this higher to allow for the possibility of more than `min_sum2_count`
    /// messages to be processed in the `sum2` phase.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// min_sum2_time = 5
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__MIN_SUM2_TIME=5
    /// ```
    pub min_sum2_time: u64,

    /// The maximum amount of time permitted for processing messages in the `sum` phase, in seconds.
    /// Set this lower to allow for the processing of `min_sum_count` messages to time-out sooner in
    /// the `sum` phase.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// max_sum_time = 30
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__MAX_SUM_TIME=30
    /// ```
    pub max_sum_time: u64,

    /// The maximum amount of time permitted for processing messages in the `update` phase, in
    /// seconds. Set this lower to allow for the processing of `min_update_count` messages to
    /// time-out sooner in the `update` phase.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// max_update_time = 60
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__MAX_UPDATE_TIME=60
    /// ```
    pub max_update_time: u64,

    /// The maximum amount of time permitted for processing messages in the `sum2` phase, in
    /// seconds. Set this lower to allow for the processing of `min_sum2_count` messages to time-out
    /// sooner in the `sum2` phase.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// max_sum2_time = 30
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__MAX_SUM2_TIME=30
    /// ```
    pub max_sum2_time: u64,

    /// The expected fraction of participants selected for preparing and computing the aggregated
    /// masks. The value must be between `0` and `1` (i.e. `0 < sum < 1`).
    ///
    /// Additionally, it is enforced that `0 < sum + update - sum*update <= 1` to avoid pathological
    /// cases of deadlocks.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// sum = 0.01
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__SUM=0.01
    /// ```
    pub sum: f64,

    /// The expected fraction of participants selected for submitting an updated local model for
    /// aggregation. The value must be between `0` and `1` (i.e. `0 < update <= 1`). Here, `1` is
    /// included to be able to express that every participant who is not a sum participant must be
    /// an update participant.
    ///
    /// Additionally, it is enforced that `0 < sum + update - sum*update <= 1` to avoid pathological
    /// cases of deadlocks.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [pet]
    /// update = 0.1
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_PET__UPDATE=0.1
    /// ```
    pub update: f64,
}

impl PetSettings {
    /// Checks the PET settings.
    fn validate_pet(&self) -> Result<(), ValidationError> {
        self.validate_phase_counts()?;
        self.validate_phase_times()?;
        self.validate_fractions()
    }

    /// Checks the validity of phase count ranges.
    fn validate_phase_counts(&self) -> Result<(), ValidationError> {
        // the validate attribute only accepts literals, therefore we check the invariants here
        if MIN_SUM_COUNT <= self.min_sum_count
            && self.min_sum_count <= self.max_sum_count
            && MIN_UPDATE_COUNT <= self.min_update_count
            && self.min_update_count <= self.max_update_count
            && MIN_SUM_COUNT <= self.min_sum2_count
            && self.min_sum2_count <= self.max_sum2_count
            && self.min_sum2_count <= self.max_sum_count
            && self.max_sum2_count <= self.max_sum_count
        {
            Ok(())
        } else {
            Err(ValidationError::new("invalid phase count range(s)"))
        }
    }

    /// Checks the validity of phase time ranges.
    fn validate_phase_times(&self) -> Result<(), ValidationError> {
        if self.min_sum_time <= self.max_sum_time
            && self.min_update_time <= self.max_update_time
            && self.min_sum2_time <= self.max_sum2_time
        {
            Ok(())
        } else {
            Err(ValidationError::new("invalid phase time range(s)"))
        }
    }

    /// Checks the validity of fraction ranges including pathological cases of deadlocks.
    fn validate_fractions(&self) -> Result<(), ValidationError> {
        if 0. < self.sum
            && self.sum < 1.
            && 0. < self.update
            && self.update <= 1.
            && 0. < self.sum + self.update - self.sum * self.update
            && self.sum + self.update - self.sum * self.update <= 1.
        {
            Ok(())
        } else {
            Err(ValidationError::new("starvation"))
        }
    }
}

/// A wrapper for validate derive.
fn validate_pet(s: &PetSettings) -> Result<(), ValidationError> {
    s.validate_pet()
}

#[derive(Debug, Deserialize, Clone)]
#[cfg_attr(
    feature = "tls",
    derive(Validate),
    validate(schema(function = "validate_api"))
)]
/// REST API settings.
///
/// Requires at least one of the following arguments if the `tls` feature is enabled:
/// - `tls_certificate` together with `tls_key` for TLS server authentication
// - `tls_client_auth` for TLS client authentication
pub struct ApiSettings {
    /// The address to which the REST API should be bound.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [api]
    /// bind_address = "0.0.0.0:8081"
    /// # or
    /// bind_address = "127.0.0.1:8081"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_API__BIND_ADDRESS=127.0.0.1:8081
    /// ```
    pub bind_address: std::net::SocketAddr,

    #[cfg(feature = "tls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tls")))]
    /// The path to the server certificate to enable TLS server authentication. Leave this out to
    /// disable server authentication. If this is present, then `tls_key` must also be present.
    ///
    /// Requires the `tls` feature to be enabled.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [api]
    /// tls_certificate = path/to/tls/files/cert.pem
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_API__TLS_CERTIFICATE=path/to/tls/files/certificate.pem
    /// ```
    pub tls_certificate: Option<PathBuf>,

    #[cfg(feature = "tls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tls")))]
    /// The path to the server private key to enable TLS server authentication. Leave this out to
    /// disable server authentication. If this is present, then `tls_certificate` must also be
    /// present.
    ///
    /// Requires the `tls` feature to be enabled.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [api]
    /// tls_key = path/to/tls/files/key.rsa
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_API__TLS_KEY=path/to/tls/files/key.rsa
    /// ```
    pub tls_key: Option<PathBuf>,

    #[cfg(feature = "tls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tls")))]
    /// The path to the trust anchor to enable TLS client authentication. Leave this out to disable
    /// client authentication.
    ///
    /// Requires the `tls` feature to be enabled.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [api]
    /// tls_client_auth = path/to/tls/files/trust_anchor.pem
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_API__TLS_CLIENT_AUTH=path/to/tls/files/trust_anchor.pem
    /// ```
    pub(crate) tls_client_auth: Option<PathBuf>,
}

#[cfg(feature = "tls")]
impl ApiSettings {
    /// Checks API settings.
    fn validate_api(&self) -> Result<(), ValidationError> {
        match (&self.tls_certificate, &self.tls_key, &self.tls_client_auth) {
            (Some(_), Some(_), _) | (None, None, Some(_)) => Ok(()),
            _ => Err(ValidationError::new("invalid tls settings")),
        }
    }
}

/// A wrapper for validate derive.
#[cfg(feature = "tls")]
fn validate_api(s: &ApiSettings) -> Result<(), ValidationError> {
    s.validate_api()
}

#[derive(Debug, Validate, Deserialize, Clone, Copy)]
/// Masking settings.
pub struct MaskSettings {
    /// The order of the finite group.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [mask]
    /// group_type = "Integer"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_MASK__GROUP_TYPE=Integer
    /// ```
    pub group_type: GroupType,

    /// The data type of the numbers to be masked.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [mask]
    /// data_type = "F32"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_MASK__DATA_TYPE=F32
    /// ```
    pub data_type: DataType,

    /// The bounds of the numbers to be masked.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [mask]
    /// bound_type = "B0"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_MASK__BOUND_TYPE=B0
    /// ```
    pub bound_type: BoundType,

    /// The maximum number of models to be aggregated.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [mask]
    /// model_type = "M3"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_MASK__MODEL_TYPE=M3
    /// ```
    pub model_type: ModelType,
}

impl From<MaskSettings> for MaskConfig {
    fn from(
        MaskSettings {
            group_type,
            data_type,
            bound_type,
            model_type,
        }: MaskSettings,
    ) -> MaskConfig {
        MaskConfig {
            group_type,
            data_type,
            bound_type,
            model_type,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
/// Model settings.
pub struct ModelSettings {
    /// The expected length of the model. The model length corresponds to the number of elements.
    /// This value is used to validate the uniform length of the submitted models/masks.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [model]
    /// length = 100
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_MODEL__LENGTH=100
    /// ```
    pub length: usize,
}

#[derive(Debug, Deserialize, Validate)]
/// Metrics settings.
pub struct MetricsSettings {
    #[validate]
    /// Settings for the InfluxDB backend.
    pub influxdb: InfluxSettings,
}

#[derive(Debug, Deserialize, Validate)]
/// InfluxDB settings.
pub struct InfluxSettings {
    #[validate(url)]
    /// The URL where InfluxDB is running.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [metrics.influxdb]
    /// url = "http://localhost:8086"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_METRICS__INFLUXDB__URL=http://localhost:8086
    /// ```
    pub url: String,

    /// The InfluxDB database name.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [metrics.influxdb]
    /// db = "test"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_METRICS__INFLUXDB__DB=test
    /// ```
    pub db: String,
}

#[derive(Debug, Deserialize)]
/// Redis settings.
pub struct RedisSettings {
    /// The URL where Redis is running.
    ///
    /// The format of the URL is `redis://[<username>][:<passwd>@]<hostname>[:port][/<db>]`.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [redis]
    /// url = "redis://127.0.0.1/"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_REDIS__URL=redis://127.0.0.1/
    /// ```
    #[serde(deserialize_with = "deserialize_redis_url")]
    pub url: ConnectionInfo,
}

fn deserialize_redis_url<'de, D>(deserializer: D) -> Result<ConnectionInfo, D::Error>
where
    D: Deserializer<'de>,
{
    struct ConnectionInfoVisitor;

    impl<'de> Visitor<'de> for ConnectionInfoVisitor {
        type Value = ConnectionInfo;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(
                formatter,
                "redis://[<username>][:<passwd>@]<hostname>[:port][/<db>]"
            )
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            value
                .into_connection_info()
                .map_err(|_| de::Error::invalid_value(serde::de::Unexpected::Str(value), &self))
        }
    }

    deserializer.deserialize_str(ConnectionInfoVisitor)
}

#[derive(Debug, Deserialize, Validate)]
/// Trust anchor settings.
pub struct TrustAnchorSettings {
    #[cfg(feature = "iota")]
    #[validate]
    /// Settings for the IOTA backend.
    pub iota: IotaSettings,
}

#[cfg(not(feature = "iota"))]
// Default value for the global models bucket
impl Default for TrustAnchorSettings {
    fn default() -> Self {
        Self {}
    }
}

#[derive(Debug, Deserialize)]
/// Logging settings.
pub struct LoggingSettings {
    /// A comma-separated list of logging directives. More information about logging directives
    /// can be found [here].
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [log]
    /// filter = "info"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_LOG__FILTER=info
    /// ```
    ///
    /// [here]: https://docs.rs/tracing-subscriber/0.2.15/tracing_subscriber/filter/struct.EnvFilter.html#directives
    #[serde(deserialize_with = "deserialize_env_filter")]
    pub filter: EnvFilter,
}

fn deserialize_env_filter<'de, D>(deserializer: D) -> Result<EnvFilter, D::Error>
where
    D: Deserializer<'de>,
{
    struct EnvFilterVisitor;

    impl<'de> Visitor<'de> for EnvFilterVisitor {
        type Value = EnvFilter;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "a valid tracing filter directive: https://docs.rs/tracing-subscriber/0.2.6/tracing_subscriber/filter/struct.EnvFilter.html#directives")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            EnvFilter::try_new(value)
                .map_err(|_| de::Error::invalid_value(serde::de::Unexpected::Str(value), &self))
        }
    }

    deserializer.deserialize_str(EnvFilterVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Default for PetSettings {
        fn default() -> Self {
            Self {
                min_sum_count: 10,
                min_update_count: 100,
                min_sum2_count: 10,
                max_sum_count: 100,
                max_update_count: 10000,
                max_sum2_count: 100,
                min_sum_time: 0,
                min_update_time: 0,
                min_sum2_time: 0,
                max_sum_time: 604800,
                max_update_time: 604800,
                max_sum2_time: 604800,
                sum: 0.01,
                update: 0.1,
            }
        }
    }

    impl Default for MaskSettings {
        fn default() -> Self {
            Self {
                group_type: GroupType::Prime,
                data_type: DataType::F32,
                bound_type: BoundType::B0,
                model_type: ModelType::M3,
            }
        }
    }

    #[test]
    fn test_settings_new() {
        assert!(Settings::new("../../configs/config.toml").is_ok());
        assert!(Settings::new("").is_err());
    }

    #[test]
    fn test_validate_pet() {
        assert!(PetSettings::default().validate_pet().is_ok());
    }

    #[test]
    fn test_validate_pet_counts() {
        assert_eq!(MIN_SUM_COUNT, 1);
        assert_eq!(MIN_UPDATE_COUNT, 3);
        assert!(PetSettings {
            min_sum_count: 0,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
        assert!(PetSettings {
            min_update_count: 2,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
        assert!(PetSettings {
            min_sum2_count: 0,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
        assert!(PetSettings {
            min_sum_count: 11,
            max_sum_count: 10,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
        assert!(PetSettings {
            min_update_count: 11,
            max_update_count: 10,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
        assert!(PetSettings {
            min_sum2_count: 11,
            max_sum2_count: 10,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
        assert!(PetSettings {
            min_sum2_count: 11,
            max_sum_count: 10,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
        assert!(PetSettings {
            max_sum2_count: 11,
            max_sum_count: 10,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn test_validate_pet_times() {
        assert!(PetSettings {
            min_sum_time: 2,
            max_sum_time: 1,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
        assert!(PetSettings {
            min_update_time: 2,
            max_update_time: 1,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
        assert!(PetSettings {
            min_sum2_time: 2,
            max_sum2_time: 1,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn test_validate_pet_fractions() {
        assert!(PetSettings {
            sum: 0.,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
        assert!(PetSettings {
            sum: 1.,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
        assert!(PetSettings {
            update: 0.,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
        assert!(PetSettings {
            update: 1. + f64::EPSILON,
            ..PetSettings::default()
        }
        .validate()
        .is_err());
    }

    #[cfg(feature = "tls")]
    #[test]
    fn test_validate_api() {
        let bind_address = ([0, 0, 0, 0], 0).into();
        let some_path = Some(std::path::PathBuf::new());

        assert!(ApiSettings {
            bind_address,
            tls_certificate: some_path.clone(),
            tls_key: some_path.clone(),
            tls_client_auth: some_path.clone(),
        }
        .validate()
        .is_ok());
        assert!(ApiSettings {
            bind_address,
            tls_certificate: some_path.clone(),
            tls_key: some_path.clone(),
            tls_client_auth: None,
        }
        .validate()
        .is_ok());
        assert!(ApiSettings {
            bind_address,
            tls_certificate: None,
            tls_key: None,
            tls_client_auth: some_path.clone(),
        }
        .validate()
        .is_ok());

        assert!(ApiSettings {
            bind_address,
            tls_certificate: some_path.clone(),
            tls_key: None,
            tls_client_auth: some_path.clone(),
        }
        .validate()
        .is_err());
        assert!(ApiSettings {
            bind_address,
            tls_certificate: None,
            tls_key: some_path.clone(),
            tls_client_auth: some_path.clone(),
        }
        .validate()
        .is_err());
        assert!(ApiSettings {
            bind_address,
            tls_certificate: some_path.clone(),
            tls_key: None,
            tls_client_auth: None,
        }
        .validate()
        .is_err());
        assert!(ApiSettings {
            bind_address,
            tls_certificate: None,
            tls_key: some_path,
            tls_client_auth: None,
        }
        .validate()
        .is_err());
        assert!(ApiSettings {
            bind_address,
            tls_certificate: None,
            tls_key: None,
            tls_client_auth: None,
        }
        .validate()
        .is_err());
    }
}
