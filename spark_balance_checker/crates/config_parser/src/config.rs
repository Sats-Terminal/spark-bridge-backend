use serde::{Deserialize, Serialize};
use spark_client::common::config::SparkOperatorConfig;

use std::{fmt::Debug, net::SocketAddr, str::FromStr};

use crate::error::ConfigParserError;
use config::{Config, Environment};
use global_utils::config_variant::ConfigVariant;
use tonic::transport::Certificate;
use tracing::{debug, instrument, trace};

const CONFIG_FOLDER_NAME: &str = "../../infrastructure/configuration";
const PRODUCTION_CONFIG_FOLDER_NAME: &str = "configuration_spark_balance_checker";
const CARGO_MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";
const DEFAULT_APP_LOCAL_BASE_FILENAME: &str = "base";
const DEFAULT_CA_FILENAME: &str = "ca.pem";

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
        let format_name = |folder_path: &str, config_folder_name: &str, filename: &str| -> String {
            format!("{folder_path}{config_folder_name}/{}.toml", filename)
        };
        if config_variant != ConfigVariant::Production {
            let _ = dotenv::dotenv().ok();
        }
        let config = match &config_variant {
            ConfigVariant::Production
            | ConfigVariant::Local
            | ConfigVariant::CustomFilepath(_)
            | ConfigVariant::CustomName(_) => {
                let (path_to_another_config_to_merge, path_to_base) = match config_variant {
                    ConfigVariant::Production => {
                        let (folder_path, config_folder_name) = ("/", PRODUCTION_CONFIG_FOLDER_NAME);
                        (
                            format_name(folder_path, config_folder_name, &config_variant.to_string()),
                            format_name(folder_path, config_folder_name, DEFAULT_APP_LOCAL_BASE_FILENAME),
                        )
                    }
                    ConfigVariant::Local => {
                        let (folder_path, config_folder_name) =
                            (format!("{}/", get_cargo_manifest_dir()), CONFIG_FOLDER_NAME);
                        (
                            format_name(&folder_path, config_folder_name, &config_variant.to_string()),
                            format_name(&folder_path, config_folder_name, DEFAULT_APP_LOCAL_BASE_FILENAME),
                        )
                    }
                    ConfigVariant::CustomFilepath(file_path) => {
                        let (folder_path, config_folder_name) =
                            (format!("{}/", get_cargo_manifest_dir()), CONFIG_FOLDER_NAME);
                        (
                            file_path,
                            format_name(&folder_path, config_folder_name, DEFAULT_APP_LOCAL_BASE_FILENAME),
                        )
                    }
                    ConfigVariant::CustomName(name) => {
                        let (folder_path, config_folder_name) =
                            (format!("{}/", get_cargo_manifest_dir()), CONFIG_FOLDER_NAME);
                        (
                            format_name(&folder_path, config_folder_name, &name),
                            format_name(&folder_path, config_folder_name, DEFAULT_APP_LOCAL_BASE_FILENAME),
                        )
                    }
                    ConfigVariant::OnlyOneFilepath(_) => unreachable!(),
                };
                debug!(path = %path_to_another_config_to_merge, config_path = %path_to_base);
                Config::builder()
                    .add_source(config::File::with_name(&path_to_base))
                    .add_source(config::File::with_name(&path_to_another_config_to_merge))
                    .add_source(Environment::with_prefix("config").separator("_").keep_prefix(false))
                    .build()?
                    .try_deserialize::<ServerConfig>()?
            }
            ConfigVariant::OnlyOneFilepath(filepath) => {
                debug!(onepath = %filepath);
                Config::builder()
                    .add_source(config::File::with_name(filepath))
                    .add_source(Environment::with_prefix("config").separator("_").keep_prefix(false))
                    .build()?
                    .try_deserialize::<ServerConfig>()?
            }
        };
        Ok(config)
    }
}

pub fn obtain_tonic_ca_cert(config_variant: ConfigVariant) -> crate::error::Result<Certificate> {
    trace!("Initializing, {config_variant}...");
    let _ = dotenv::dotenv().ok();
    let format_name = |folder_path: &str, config_folder_name: &str| -> String {
        format!("{folder_path}{config_folder_name}/{DEFAULT_CA_FILENAME}")
    };
    let path_to_ca = match config_variant {
        ConfigVariant::Production => {
            let (folder_path, config_folder_name) = ("/", PRODUCTION_CONFIG_FOLDER_NAME);
            format_name(folder_path, config_folder_name)
        }
        ConfigVariant::Local => {
            let (folder_path, config_folder_name) = (format!("{}/", get_cargo_manifest_dir()), CONFIG_FOLDER_NAME);
            format_name(&folder_path, config_folder_name)
        }
        ConfigVariant::CustomFilepath(filepath) | ConfigVariant::OnlyOneFilepath(filepath) => filepath,
        ConfigVariant::CustomName(name) => {
            let (folder_path, config_folder_name) = (format!("{}/", get_cargo_manifest_dir()), name);
            format_name(&folder_path, &config_folder_name)
        }
    };
    debug!("Path with certificate to resolve: path_to_ca: '{path_to_ca}'",);
    let file = std::fs::read(path_to_ca.clone())
        .map_err(|err| ConfigParserError::FailedToOpenFile { err, path: path_to_ca })?;
    Ok(Certificate::from_pem(file))
}

pub fn get_cargo_manifest_dir() -> String {
    std::env::var(CARGO_MANIFEST_DIR).unwrap()
}
