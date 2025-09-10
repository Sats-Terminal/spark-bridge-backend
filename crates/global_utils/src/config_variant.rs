use serde::{Deserialize, Serialize};
use std::env::VarError;
use tracing::{info, instrument};

pub const APP_CONFIGURATION_NAME: &str = "APP_ENVIRONMENT";
pub const DEFAULT_APP_PRODUCTION_CONFIG_NAME: &str = "production";
pub const DEFAULT_APP_LOCAL_CONFIG_NAME: &str = "local";

/// Uses `json` to parse its configuration from String
#[derive(Debug, Clone, strum::Display, Serialize, Deserialize, Default, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConfigVariant {
    /// This entry is using a `production` name for configuration to perform search for appropriate configs `[merges 'base.toml', and 'additional_file.toml']`
    #[strum(serialize = "production")]
    Production,
    /// This entry is using a `local` name for configuration to perform search for appropriate configs `[merges 'base.toml', and 'additional_file.toml']`
    #[strum(serialize = "local")]
    #[default]
    Local,
    /// This entry is used a custom **filepath** in which can be found appropriate configuration `[merges 'base.toml', and 'additional_file.toml']`
    #[strum(serialize = "custom_filepath")]
    CustomFilepath(String),
    /// Enum entry is used for using custom name, but search is performed in the same directories `[merges 'base.toml', and 'additional_file.toml']`
    #[strum(serialize = "custom_name")]
    CustomName(String),
    /// Enum entry is using ONLY 1 configuration path from given path `[uses only 'given_file.ext' for parsing config]`
    #[strum(serialize = "only_one_filepath")]
    OnlyOneFilepath(String),
}

impl ConfigVariant {
    #[instrument(level = "trace", ret)]
    pub fn init() -> ConfigVariant {
        info!("{:?}", std::env::var(APP_CONFIGURATION_NAME));
        match std::env::var(APP_CONFIGURATION_NAME) {
            Ok(app_config) => {
                if app_config == ConfigVariant::Production.to_string() {
                    ConfigVariant::Production
                } else if app_config == ConfigVariant::Local.to_string() {
                    ConfigVariant::Local
                } else {
                    serde_json::from_str::<ConfigVariant>(&app_config).unwrap_or(ConfigVariant::default())
                }
            }
            Err(_) => ConfigVariant::default(),
        }
    }
}
