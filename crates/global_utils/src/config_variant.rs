use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

pub const APP_CONFIGURATION_NAME: &str = "APP_ENVIRONMENT";
pub const DEFAULT_APP_PRODUCTION_CONFIG_NAME: &str = "production";
pub const DEFAULT_APP_LOCAL_CONFIG_NAME: &str = "local";

#[derive(Debug, Copy, Clone, strum::Display, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigVariant {
    #[strum(serialize = "production")]
    Production,
    #[strum(serialize = "local")]
    Local,
}

impl ConfigVariant {
    #[instrument(level = "trace", ret)]
    pub fn init() -> ConfigVariant {
        info!("{:?}", std::env::var(APP_CONFIGURATION_NAME));
        if let Ok(x) = std::env::var(APP_CONFIGURATION_NAME)
            && x == ConfigVariant::Production.to_string()
        {
            ConfigVariant::Production
        } else {
            ConfigVariant::Local
        }
    }
}
