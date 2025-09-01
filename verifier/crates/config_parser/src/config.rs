use config::{Config, Environment};
use global_utils::config_variant::ConfigVariant;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::str::FromStr;
use tracing::{debug, instrument};

const CONFIG_FOLDER_NAME: &str = "../../infrastructure/configuration";
const PRODUCTION_CONFIG_FOLDER_NAME: &str = "configuration_verifier";
const CARGO_MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";
const DEFAULT_APP_LOCAL_BASE_FILENAME: &str = "base.toml";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct AppConfig {
    #[serde(rename = "http_server_ip")]
    pub ip: String,
    #[serde(rename = "http_server_port")]
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct BtcIndexerConfig {
    pub address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SparkBalanceCheckerConfig {
    pub address: String,
}

/// Struct used for initialization of different kinds of configurations
///
/// Example of using local configuration:
/// ```rust
/// use config_parser::config::{ConfigVariant, DEFAULT_APP_LOCAL_CONFIG_NAME, ServerConfig};
/// let config = ServerConfig::init_config(ConfigVariant::Local);
/// assert!(config.is_ok())
/// ```
// Example of using production configuration:
// ```
// use config_parser::config::{ConfigVariant, DEFAULT_APP_PRODUCTION_CONFIG_NAME, ServerConfig};
// let config = ServerConfig::init_config(ConfigVariant::Production);
// assert!(config.is_ok())
// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(rename(deserialize = "application"))]
    pub server: AppConfig,
    #[serde(rename = "btc_indexer_url_config")]
    pub btc_indexer: BtcIndexerConfig,
    #[serde(rename = "spark_balance_checker_url_config")]
    pub spark_balance_checker: SparkBalanceCheckerConfig,
}

pub fn get_cargo_manifest_dir() -> String {
    std::env::var(CARGO_MANIFEST_DIR).unwrap()
}

impl AppConfig {
    #[inline]
    pub fn get_app_binding_url(&self) -> crate::error::Result<SocketAddr> {
        Ok(SocketAddr::from_str(&format!("{}:{}", self.ip, self.port))?)
    }
}

impl ServerConfig {
    #[instrument(level = "debug", ret)]
    pub fn init_config(config_variant: ConfigVariant) -> crate::error::Result<Self> {
        println!("Initializing, {config_variant}...");
        let (folder_path, config_folder_name) = match config_variant {
            ConfigVariant::Production => ("/".to_string(), PRODUCTION_CONFIG_FOLDER_NAME),
            ConfigVariant::Local => {
                let _ = dotenv::dotenv().ok().unwrap();
                (format!("{}/", get_cargo_manifest_dir()), CONFIG_FOLDER_NAME)
            }
        };
        debug!("Configuration folder lookup path: {folder_path}");
        println!(
            "Path: {}",
            format!("{folder_path}{config_folder_name}/{DEFAULT_APP_LOCAL_BASE_FILENAME}")
        );
        Ok(Config::builder()
            .add_source(config::File::with_name(&format!(
                "{folder_path}{config_folder_name}/{DEFAULT_APP_LOCAL_BASE_FILENAME}"
            )))
            .add_source(config::File::with_name(&format!(
                "{folder_path}{config_folder_name}/{}.toml",
                config_variant
            )))
            .add_source(Environment::default())
            .build()?
            .try_deserialize::<ServerConfig>()?)
    }
}
