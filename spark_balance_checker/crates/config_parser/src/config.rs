use serde::{Deserialize, Serialize};
use spark_client::SparkOperatorConfig;

use std::{fmt::Debug, io, net::SocketAddr, str::FromStr};

use crate::error::ConfigParserError;
use config::{Config, Environment};
use global_utils::config_variant::ConfigVariant;
use tonic::transport::Certificate;
use tracing::{debug, instrument, trace};

const CONFIG_FOLDER_NAME: &str = "../../infrastructure/configuration";
const PRODUCTION_CONFIG_FOLDER_NAME: &str = "configuration_spark_balance_checker";
const CARGO_MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";
const DEFAULT_APP_LOCAL_BASE_FILENAME: &str = "base.toml";

/// Struct used for initialization of different kinds of configurations
///
/// Example of using local configuration:
/// ```rust
/// use spark_balance_checker_config_parser::config::{ServerConfig};
/// use global_utils::config_variant::ConfigVariant;
/// let config = ServerConfig::init_config(ConfigVariant::Local);
/// assert!(config.is_ok())
/// ```
// Example of using production configuration:
// ```
// use spark_balance_checker_config_parser::config::{ServerConfig};
// use global_utils::config_variant::ConfigVariant;
// let config = ServerConfig::init_config(ConfigVariant::Production);
// assert!(config.is_ok())
// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(rename(deserialize = "application"))]
    pub app_config: AppConfig,
    #[serde(rename(deserialize = "spark_operators"))]
    pub spark_operators: Vec<SparkOperatorConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(rename(deserialize = "http_server_ip"))]
    pub ip: String,
    #[serde(rename(deserialize = "http_server_port"))]
    pub port: u16,
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
                let _ = dotenv::dotenv().ok();
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

pub fn obtain_tonic_ca_cert(config_variant: ConfigVariant) -> crate::error::Result<Certificate> {
    trace!("Initializing, {config_variant}...");
    let (folder_path, config_folder_name) = match config_variant {
        ConfigVariant::Production => ("/".to_string(), PRODUCTION_CONFIG_FOLDER_NAME),
        ConfigVariant::Local => {
            let _ = dotenv::dotenv().ok();
            (format!("{}/", get_cargo_manifest_dir()), CONFIG_FOLDER_NAME)
        }
    };
    debug!("Configuration folder lookup path: {folder_path}");
    let path_to_ca = format!("{folder_path}{config_folder_name}/{DEFAULT_APP_LOCAL_BASE_FILENAME}");
    trace!("Path with certificate to resolve: path_to_ca: '{path_to_ca}'",);
    let file = std::fs::read(path_to_ca.clone())
        .map_err(|err| ConfigParserError::FailedToOpenFile { err, path: path_to_ca })?;
    Ok(Certificate::from_pem(file))
}

pub fn get_cargo_manifest_dir() -> String {
    std::env::var(CARGO_MANIFEST_DIR).unwrap()
}
