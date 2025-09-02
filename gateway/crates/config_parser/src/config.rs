use config::{Config, Environment};
use global_utils::config_variant::ConfigVariant;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::str::FromStr;
use tracing::{debug, instrument, trace};

const CONFIG_FOLDER_NAME: &str = "../../infrastructure/configuration";
const PRODUCTION_CONFIG_FOLDER_NAME: &str = "configuration_gateway";
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
pub struct VerifierConfig {
    pub id: u64,
    pub address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(transparent)]
pub struct VerifiersConfig(Vec<VerifierConfig>);

/// Struct used for initialization of different kinds of configurations
///
/// Example of using local configuration:
/// ```rust
/// use global_utils::config_variant::ConfigVariant;
/// use gateway_config_parser::config::ServerConfig;
/// let config = ServerConfig::init_config(ConfigVariant::Local);
/// assert!(config.is_ok())
/// ```
// Example of using production configuration:
// ```
// use global_utils::config_variant::ConfigVariant;
// use gateway_config_parser::config::ServerConfig;
// let config = ServerConfig::init_config(ConfigVariant::Production);
// assert!(config.is_ok())
// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(rename(deserialize = "application"))]
    pub server: AppConfig,
    #[serde(rename = "verifiers_config")]
    pub verifiers: VerifiersConfig,
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
        trace!("Initializing, {config_variant}...");
        let (folder_path, config_folder_name) = match config_variant {
            ConfigVariant::Production => ("/".to_string(), PRODUCTION_CONFIG_FOLDER_NAME),
            ConfigVariant::Local => {
                let _ = dotenv::dotenv().ok().unwrap();
                (format!("{}/", get_cargo_manifest_dir()), CONFIG_FOLDER_NAME)
            }
        };
        debug!("Configuration folder lookup path: {folder_path}");
        let (path_to_base, path_to_another_config_to_merge) = (
            format!("{folder_path}{config_folder_name}/{DEFAULT_APP_LOCAL_BASE_FILENAME}"),
            format!("{folder_path}{config_folder_name}/{}.toml", config_variant),
        );
        trace!(
            "Paths to resolve: path_to_base: '{path_to_base}', path_to_another_config: '{path_to_another_config_to_merge}'",
        );
        Ok(Config::builder()
            .add_source(config::File::with_name(&path_to_base))
            .add_source(config::File::with_name(&path_to_another_config_to_merge))
            .add_source(Environment::with_prefix("config").separator("_").keep_prefix(false))
            .build()?
            .try_deserialize::<ServerConfig>()?)
    }
}