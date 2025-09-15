use bitcoin::Network as BtcNetwork;
use config::{Config, Environment};
use global_utils::config_variant::ConfigVariant;
use serde::{Deserialize, Serialize};
use spark_client::utils::spark_address::Network as SparkNetwork;
use std::net::SocketAddr;
use std::str::FromStr;
use tracing::{debug, instrument, trace};

const CONFIG_FOLDER_NAME: &str = "../../infrastructure/configuration";
const PRODUCTION_CONFIG_FOLDER_NAME: &str = "configuration_gateway";
const CARGO_MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";
const DEFAULT_APP_LOCAL_BASE_FILENAME: &str = "base";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct AppConfig {
    #[serde(rename = "http_server_ip")]
    pub ip: String,
    #[serde(rename = "http_server_port")]
    pub port: u16,
    #[serde(rename = "btc_network")]
    pub btc_network: BtcNetwork,
    #[serde(rename = "spark_network")]
    pub spark_network: SparkNetwork,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct VerifierConfig {
    pub id: u16,
    pub address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct AggregatorConfig {
    pub threshold: u16,
    pub total_participants: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct FlowProcessorConfig {
    pub cancellation_retries: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct PrivateApiConfig {
    #[serde(rename = "http_server_ip")]
    pub ip: String,
    #[serde(rename = "http_server_port")]
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(transparent)]
pub struct VerifiersConfig(pub Vec<VerifierConfig>);

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
    #[serde(rename = "aggregator_config")]
    pub aggregator: AggregatorConfig,
    #[serde(rename = "flow_processor_config")]
    pub flow_processor: FlowProcessorConfig,
    #[serde(rename = "private_api_config")]
    pub flow_processor_private_api: PrivateApiConfig,
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

impl PrivateApiConfig {
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
