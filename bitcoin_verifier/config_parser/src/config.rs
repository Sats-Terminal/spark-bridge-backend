use std::{fmt::Debug, net::SocketAddr, str::FromStr};

use config::{Config, Environment};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

const CONFIG_FOLDER_NAME: &str = "configuration";
const CARGO_MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";
const PATH_TO_CONFIGS: &str = "../../infrastructure/";
pub const APP_CONFIGURATION_NAME: &str = "APP_ENVIRONMENT";
pub const SSH_PRIVATE_KEY_PATH: &str = "SSH_PRIVATE_KEY_PATH";
pub const DEFAULT_APP_PRODUCTION_CONFIG_NAME: &str = "production";
const DEFAULT_APP_LOCAL_BASE_FILENAME: &str = "base.toml";
pub const DEFAULT_APP_LOCAL_CONFIG_NAME: &str = "local";

/// Struct used for initialization of different kinds of configurations
///
/// Example of using local configuration:
/// ```rust
/// use config_parser::config::{ConfigVariant, DEFAULT_APP_LOCAL_CONFIG_NAME, ServerConfig};
/// let config = ServerConfig::init_config(ConfigVariant::Local);
/// println!("{config:?}");
/// assert!(config.is_ok())
/// ```
/// Example of using production configuration:
/// ```rust
/// use config_parser::config::{ConfigVariant, DEFAULT_APP_PRODUCTION_CONFIG_NAME, ServerConfig};
/// let config = ServerConfig::init_config(ConfigVariant::Production);
/// assert!(config.is_ok())
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(rename(deserialize = "application"))]
    pub app_config: AppConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub http_server_ip: String,
    pub http_server_port: u16,
}

#[derive(Debug, Copy, Clone, strum::Display, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigVariant {
    #[strum(serialize = "production")]
    Production,
    #[strum(serialize = "local")]
    Local,
}

impl AppConfig {
    #[inline]
    pub fn get_app_binding_url(&self) -> crate::error::Result<SocketAddr> {
        Ok(SocketAddr::from_str(&format!(
            "{}:{}",
            self.http_server_ip, self.http_server_port
        ))?)
    }
}

impl ServerConfig {
    #[instrument(level = "debug", ret)]
    pub fn init_config(config_variant: ConfigVariant) -> crate::error::Result<Self> {
        let folder_path = match config_variant {
            ConfigVariant::Production => "/".to_string(),
            ConfigVariant::Local => {
                let _ = dotenv::dotenv().ok().unwrap();
                format!("{}/{PATH_TO_CONFIGS}", get_cargo_manifest_dir())
            }
        };
        debug!("Configuration folder lookup path: {folder_path}");
        Ok(Config::builder()
            .add_source(config::File::with_name(&format!(
                "{folder_path}{CONFIG_FOLDER_NAME}/{DEFAULT_APP_LOCAL_BASE_FILENAME}"
            )))
            .add_source(config::File::with_name(&format!(
                "{folder_path}{CONFIG_FOLDER_NAME}/{}.toml",
                config_variant
            )))
            .add_source(Environment::with_prefix("config").separator("_").keep_prefix(false))
            .build()?
            .try_deserialize::<ServerConfig>()?)
    }
}

pub fn get_cargo_manifest_dir() -> String {
    std::env::var(CARGO_MANIFEST_DIR).unwrap()
}

#[instrument(level = "trace", ret)]
pub fn get_app_config_val() -> ConfigVariant {
    if let Ok(x) = std::env::var(APP_CONFIGURATION_NAME)
        && x == crate::config::ConfigVariant::Production.to_string()
    {
        ConfigVariant::Production
    } else {
        ConfigVariant::Local
    }
}
