use iota::client::builder::Network;
use once_cell::sync::Lazy;
use redis::ConnectionInfo;
use regex::Regex;
use serde::Deserialize;
use validator::Validate;

use super::deserialize_redis_url;

static IOTA_AUTHOR_SEED: Lazy<Regex> = Lazy::new(|| regex::Regex::new("^[A-Z9]{1,}$").unwrap());

/// IOTA client settings
#[derive(Debug, Validate, Deserialize)]
pub struct IotaSettings {
    #[validate(url)]
    /// The URL where the IOTA node is running.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [trust_anchor.iota]
    /// url = "https://nodes.devnet.iota.org:443"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_TRUST_ANCHOR__IOTA__URL=https://nodes.devnet.iota.org:443
    /// ```
    pub url: String,
    /// The network to which the node belongs.
    /// You can choose from: `Mainnet`, `Devnet` or `Comnet`.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [trust_anchor.iota]
    /// network = "Devnet"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_TRUST_ANCHOR__IOTA__NETWORK=Devnet
    /// ```
    pub network: Network,
    /// The seed of the author. Allowed characters are: `A-Z` and `9`.
    ///
    /// See [here](https://docs.iota.org/docs/channels/1.3/guides/creating-a-new-channel)
    /// for more information about the author seed.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [trust_anchor.iota]
    /// author_seed = "XAYN9IOTA9AUTHOR9SEED999"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_TRUST_ANCHOR__IOTA__AUTHOR_SEED=XAYN9IOTA9AUTHOR9SEED999
    /// ```
    #[validate(regex = "IOTA_AUTHOR_SEED")]
    pub author_seed: String,
    /// The password with which the author state is de / encrypted. The password must be at
    /// least 10 characters long.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [trust_anchor.iota]
    /// author_state_pwd = "xaynet_iota_test_password"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_TRUST_ANCHOR__IOTA__AUTHOR_STATE_PWD=xaynet_iota_test_password
    /// ```
    #[validate(length(min = 10))]
    pub author_state_pwd: String,
    ///  A Redis store in which the author state is persisted.
    pub store: RedisStoreSettings,
}

#[derive(Debug, Deserialize)]
/// Redis store settings.
pub struct RedisStoreSettings {
    /// The URL where Redis is running.
    ///
    /// The format of the URL is `redis://[<username>][:<passwd>@]<hostname>[:port][/<db>]`.
    ///
    /// # Examples
    ///
    /// **TOML**
    /// ```text
    /// [trust_anchor.iota.store]
    /// url = "redis://127.0.0.1/"
    /// ```
    ///
    /// **Environment variable**
    /// ```text
    /// XAYNET_TRUST_ANCHOR__IOTA__STORE__URL=redis://127.0.0.1/
    /// ```
    #[serde(deserialize_with = "deserialize_redis_url")]
    pub url: ConnectionInfo,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_author_seed_regex() {
        assert_eq!(IOTA_AUTHOR_SEED.is_match(""), false);
        assert_eq!(IOTA_AUTHOR_SEED.is_match("99999AASKJSFHSJFJKSFKJL"), true);
        assert_eq!(
            IOTA_AUTHOR_SEED.is_match("99999AASKJSFHSJFJKSFKJL9999999"),
            true
        );
        assert_eq!(
            IOTA_AUTHOR_SEED.is_match("1AASKJSFHSJFJKSFKJL9999999"),
            false
        );
        assert_eq!(
            IOTA_AUTHOR_SEED.is_match("9aASKJSFHSJFJKSFKJL9999999"),
            false
        );
    }
}
