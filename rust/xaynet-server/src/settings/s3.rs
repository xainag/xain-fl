//! S3 settings.

use std::fmt;

use fancy_regex::Regex;
use rusoto_core::Region;
use serde::{
    de::{self, value, Deserializer, Visitor},
    Deserialize,
};
use validator::{Validate, ValidationError};

#[derive(Debug, Validate, Deserialize)]
/// S3 settings.
pub struct S3Settings {
    /// The [access key ID](https://docs.aws.amazon.com/general/latest/gr/aws-sec-cred-types.html).
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [s3]
    /// access_key = "AKIAIOSFODNN7EXAMPLE"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_S3__ACCESS_KEY=AKIAIOSFODNN7EXAMPLE
    /// ```
    pub access_key: String,

    /// The [secret access key](https://docs.aws.amazon.com/general/latest/gr/aws-sec-cred-types.html).
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [s3]
    /// secret_access_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_S3__SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
    /// ```
    pub secret_access_key: String,

    /// The Regional AWS endpoint.
    ///
    /// The region is specified using the [Region code](https://docs.aws.amazon.com/general/latest/gr/rande.html#regional-endpoints)
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [s3]
    /// region = ["eu-west-1"]
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_S3__REGION="eu-west-1"
    /// ```
    ///
    /// To connect to AWS-compatible services such as Minio, you need to specify a custom region.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [s3]
    /// region = ["minio", "http://localhost:8000"]
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_S3__REGION="minio http://localhost:8000"
    /// ```
    #[serde(deserialize_with = "deserialize_s3_region")]
    pub region: Region,
    #[validate]
    #[serde(default)]
    pub buckets: S3BucketsSettings,
}

#[derive(Debug, Validate, Deserialize)]
/// S3 buckets settings.
pub struct S3BucketsSettings {
    /// The bucket name in which the global models are stored.
    /// Defaults to `global-models`.
    ///
    /// Please follow the [rules for bucket naming](https://docs.aws.amazon.com/AmazonS3/latest/dev/BucketRestrictions.html)
    /// when creating the name.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [s3.buckets]
    /// global_models = "global-models"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_S3__BUCKETS__GLOBAL_MODELS="global-models"
    /// ```
    #[validate(custom = "validate_s3_bucket_name")]
    pub global_models: String,
}

// Default value for the global models bucket
impl Default for S3BucketsSettings {
    fn default() -> Self {
        Self {
            global_models: String::from("global-models"),
        }
    }
}

// Validates the bucket name
// [Rules for AWS bucket naming](https://docs.aws.amazon.com/AmazonS3/latest/dev/BucketRestrictions.html)
fn validate_s3_bucket_name(bucket_name: &str) -> Result<(), ValidationError> {
    // https://stackoverflow.com/questions/50480924/regex-for-s3-bucket-name#comment104807676_58248645
    // I had to use fancy_regex here because the std regex does not support `look-around`
    let re =
        Regex::new(r"(?!^(\d{1,3}\.){3}\d{1,3}$)(^[a-z0-9]([a-z0-9-]*(\.[a-z0-9])?)*$(?<!\-))")
            .unwrap();
    match re.is_match(bucket_name) {
        Ok(true) => Ok(()),
        Ok(false) => Err(ValidationError::new("invalid bucket name\n See here: https://docs.aws.amazon.com/AmazonS3/latest/dev/BucketRestrictions.html")),
        // something went wrong with the regex engine
        Err(_) => Err(ValidationError::new("can not validate bucket name")),
    }
}

// A small wrapper to support the list type for environment variable values.
// config-rs always converts a environment variable value to a string
// https://github.com/mehcode/config-rs/blob/master/src/env.rs#L114 .
// Strings however, are not supported by the deserializer of rusoto_core::Region (only sequences).
// Therefore we use S3RegionVisitor to implement `visit_str` and thus support
// the deserialization of rusoto_core::Region from strings.
fn deserialize_s3_region<'de, D>(deserializer: D) -> Result<Region, D::Error>
where
    D: Deserializer<'de>,
{
    struct S3RegionVisitor;

    impl<'de> Visitor<'de> for S3RegionVisitor {
        type Value = Region;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("sequence of \"name Optional<endpoint>\"")
        }

        // FIXME: a copy of https://rusoto.github.io/rusoto/src/rusoto_core/region.rs.html#185
        // I haven't managed to create a sequence and call `self.visit_seq(seq)`.
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let mut seq = value.split_whitespace();

            let name: &str = seq
                .next()
                .ok_or_else(|| de::Error::custom("region is missing name"))?;
            let endpoint: Option<&str> = seq.next();

            match (name, endpoint) {
                (name, Some(endpoint)) => Ok(Region::Custom {
                    name: name.to_string(),
                    endpoint: endpoint.to_string(),
                }),
                (name, None) => name.parse().map_err(de::Error::custom),
            }
        }

        // delegate the call for sequences to the deserializer of rusoto_core::Region
        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            Deserialize::deserialize(value::SeqAccessDeserializer::new(seq))
        }
    }

    deserializer.deserialize_any(S3RegionVisitor)
}

#[derive(Debug, Deserialize, Validate)]
/// Restore settings.
pub struct RestoreSettings {
    /// If set to `false`, the restoring of coordinator state is prevented.
    /// Instead, the state is reset and the coordinator is started with the
    /// settings of the configuration file.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [restore]
    /// enable = true
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_RESTORE__ENABLE=false
    /// ```
    pub enable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    use config::{Config, ConfigError, Environment};
    use serial_test::serial;

    impl Settings {
        fn load_from_str(string: &str) -> Result<Self, ConfigError> {
            let mut config = Config::new();
            config.merge(config::File::from_str(string, config::FileFormat::Toml))?;
            config.merge(Environment::with_prefix("xaynet").separator("__"))?;
            config.try_into()
        }
    }

    struct ConfigBuilder {
        config: String,
    }

    impl ConfigBuilder {
        fn new() -> Self {
            Self {
                config: String::new(),
            }
        }

        fn build(self) -> String {
            self.config
        }

        fn with_log(mut self) -> Self {
            let log = r#"
            [log]
            filter = "xaynet=debug,http=warn,info"
            "#;

            self.config.push_str(log);
            self
        }

        fn with_api(mut self) -> Self {
            let api = r#"
            [api]
            bind_address = "127.0.0.1:8081"
            tls_certificate = "/app/ssl/tls.pem"
            tls_key = "/app/ssl/tls.key"
            "#;

            self.config.push_str(api);
            self
        }

        fn with_pet(mut self) -> Self {
            let pet = r#"
            [pet]
            min_sum_count = 1
            min_update_count = 3
            min_sum2_count = 1
            max_sum_count = 100
            max_update_count = 10000
            max_sum2_count = 100
            min_sum_time = 5
            min_update_time = 10
            min_sum2_time = 5
            max_sum_time = 3600
            max_update_time = 3600
            max_sum2_time = 3600
            sum = 0.5
            update = 0.9
            "#;

            self.config.push_str(pet);
            self
        }

        fn with_mask(mut self) -> Self {
            let mask = r#"
            [mask]
            group_type = "Prime"
            data_type = "F32"
            bound_type = "B0"
            model_type = "M3"
            "#;

            self.config.push_str(mask);
            self
        }

        fn with_model(mut self) -> Self {
            let model = r#"
            [model]
            length = 4
            "#;

            self.config.push_str(model);
            self
        }

        fn with_metrics(mut self) -> Self {
            let metrics = r#"
            [metrics.influxdb]
            url = "http://influxdb:8086"
            db = "metrics"
            "#;

            self.config.push_str(metrics);
            self
        }

        fn with_redis(mut self) -> Self {
            let redis = r#"
            [redis]
            url = "redis://127.0.0.1/"
            "#;

            self.config.push_str(redis);
            self
        }

        fn with_s3(mut self) -> Self {
            let s3 = r#"
            [s3]
            access_key = "minio"
            secret_access_key = "minio123"
            region = ["minio", "http://localhost:9000"]
            "#;

            self.config.push_str(s3);
            self
        }

        fn with_s3_buckets(mut self) -> Self {
            let s3_buckets = r#"
            [s3.buckets]
            global_models = "global-models-toml"
            "#;

            self.config.push_str(s3_buckets);
            self
        }

        fn with_restore(mut self) -> Self {
            let restore = r#"
            [restore]
            enable = true
            "#;

            self.config.push_str(restore);
            self
        }

        fn with_trust_anchor(mut self) -> Self {
            let restore = r#"
            [trust_anchor.iota]
            url = "https://nodes.devnet.iota.org:443"
            network = "Devnet"
            local_pow = false
            author_seed = "XAYN9IOTA9AUTHOR9SEED999"
            author_state_pwd = "xaynet_iota_test_password"

            [trust_anchor.iota.store]
            url = "redis://127.0.0.1/"
            "#;

            self.config.push_str(restore);
            self
        }

        fn with_custom(mut self, custom_config: &str) -> Self {
            self.config.push_str(custom_config);
            self
        }
    }

    #[test]
    fn test_validate_s3_bucket_name() {
        // I took the examples from https://docs.aws.amazon.com/AmazonS3/latest/dev/BucketRestrictions.html

        // valid names
        assert!(validate_s3_bucket_name("docexamplebucket").is_ok());
        assert!(validate_s3_bucket_name("log-delivery-march-2020").is_ok());
        assert!(validate_s3_bucket_name("my-hosted-content").is_ok());

        // valid but not recommended names
        assert!(validate_s3_bucket_name("docexamplewebsite.com").is_ok());
        assert!(validate_s3_bucket_name("www.docexamplewebsite.com").is_ok());
        assert!(validate_s3_bucket_name("my.example.s3.bucket").is_ok());

        // invalid names
        assert!(validate_s3_bucket_name("doc_example_bucket").is_err());
        assert!(validate_s3_bucket_name("DocExampleBucket").is_err());
        assert!(validate_s3_bucket_name("doc-example-bucket-").is_err());
    }

    #[test]
    #[serial]
    fn test_s3_bucket_name_default() {
        let config = ConfigBuilder::new()
            .with_log()
            .with_api()
            .with_pet()
            .with_mask()
            .with_model()
            .with_metrics()
            .with_redis()
            .with_restore()
            .with_trust_anchor()
            .with_s3()
            .build();

        let settings = Settings::load_from_str(&config).unwrap();
        assert_eq!(
            settings.s3.buckets.global_models,
            S3BucketsSettings::default().global_models
        )
    }

    #[test]
    #[serial]
    fn test_s3_bucket_name_toml_overrides_default() {
        let config = ConfigBuilder::new()
            .with_log()
            .with_api()
            .with_pet()
            .with_mask()
            .with_model()
            .with_metrics()
            .with_redis()
            .with_restore()
            .with_trust_anchor()
            .with_s3()
            .with_s3_buckets()
            .build();

        let settings = Settings::load_from_str(&config).unwrap();
        assert_eq!(settings.s3.buckets.global_models, "global-models-toml")
    }

    #[test]
    #[serial]
    fn test_s3_bucket_name_env_overrides_toml_and_default() {
        let config = ConfigBuilder::new()
            .with_log()
            .with_api()
            .with_pet()
            .with_mask()
            .with_model()
            .with_metrics()
            .with_redis()
            .with_restore()
            .with_trust_anchor()
            .with_s3()
            .with_s3_buckets()
            .build();

        std::env::set_var("XAYNET_S3__BUCKETS__GLOBAL_MODELS", "global-models-env");
        let settings = Settings::load_from_str(&config).unwrap();
        assert_eq!(settings.s3.buckets.global_models, "global-models-env");
        std::env::remove_var("XAYNET_S3__BUCKETS__GLOBAL_MODELS");
    }

    #[test]
    #[serial]
    fn test_s3_bucket_name_env_overrides_default() {
        let config = ConfigBuilder::new()
            .with_log()
            .with_api()
            .with_pet()
            .with_mask()
            .with_model()
            .with_metrics()
            .with_redis()
            .with_restore()
            .with_trust_anchor()
            .with_s3()
            .build();

        std::env::set_var("XAYNET_S3__BUCKETS__GLOBAL_MODELS", "global-models-env");
        let settings = Settings::load_from_str(&config).unwrap();
        assert_eq!(settings.s3.buckets.global_models, "global-models-env");
        std::env::remove_var("XAYNET_S3__BUCKETS__GLOBAL_MODELS");
    }

    #[test]
    #[serial]
    fn test_s3_region_toml() {
        let region = r#"
        [s3]
        access_key = "minio"
        secret_access_key = "minio123"
        region = ["eu-west-1"]
        "#;

        let config = ConfigBuilder::new()
            .with_log()
            .with_api()
            .with_pet()
            .with_mask()
            .with_model()
            .with_metrics()
            .with_redis()
            .with_restore()
            .with_trust_anchor()
            .with_custom(region)
            .build();

        let settings = Settings::load_from_str(&config).unwrap();
        assert!(matches!(settings.s3.region, Region::EuWest1));
    }

    #[test]
    #[serial]
    fn test_s3_custom_region_toml() {
        let config = ConfigBuilder::new()
            .with_log()
            .with_api()
            .with_pet()
            .with_mask()
            .with_model()
            .with_metrics()
            .with_redis()
            .with_restore()
            .with_trust_anchor()
            .with_s3()
            .build();

        let settings = Settings::load_from_str(&config).unwrap();
        assert!(matches!(
            settings.s3.region,
            Region::Custom {
                name,
                endpoint
            } if name == "minio" && endpoint == "http://localhost:9000"
        ));
    }

    #[test]
    #[serial]
    fn test_s3_region_env() {
        let config = ConfigBuilder::new()
            .with_log()
            .with_api()
            .with_pet()
            .with_mask()
            .with_model()
            .with_metrics()
            .with_redis()
            .with_restore()
            .with_trust_anchor()
            .with_s3()
            .build();

        std::env::set_var("XAYNET_S3__REGION", "eu-west-1");
        let settings = Settings::load_from_str(&config).unwrap();
        assert!(matches!(settings.s3.region, Region::EuWest1));
        std::env::remove_var("XAYNET_S3__REGION");
    }

    #[test]
    #[serial]
    fn test_restore() {
        let no_restore = r#"
        [restore]
        enable = false
        "#;

        let config = ConfigBuilder::new()
            .with_log()
            .with_api()
            .with_pet()
            .with_mask()
            .with_model()
            .with_metrics()
            .with_redis()
            .with_s3()
            .with_trust_anchor()
            .with_custom(no_restore)
            .build();

        let settings = Settings::load_from_str(&config).unwrap();
        assert_eq!(settings.restore.enable, false);
    }

    #[test]
    #[serial]
    fn test_s3_custom_region_env() {
        let config = ConfigBuilder::new()
            .with_log()
            .with_api()
            .with_pet()
            .with_mask()
            .with_model()
            .with_metrics()
            .with_redis()
            .with_restore()
            .with_trust_anchor()
            .with_s3()
            .build();

        std::env::set_var("XAYNET_S3__REGION", "minio-env http://localhost:8000");
        let settings = Settings::load_from_str(&config).unwrap();
        assert!(matches!(
            settings.s3.region,
            Region::Custom {
                name,
                endpoint
            } if name == "minio-env" && endpoint == "http://localhost:8000"
        ));
        std::env::remove_var("XAYNET_S3__REGION");
    }
}
