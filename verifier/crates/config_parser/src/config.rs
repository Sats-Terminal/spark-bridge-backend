use bitcoin::Network;
use config::{Config, Environment};
use global_utils::common_types::Url;
use global_utils::config_variant::ConfigVariant;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::str::FromStr;
use tracing::{debug, instrument, trace};

const CONFIG_FOLDER_NAME: &str = "../../infrastructure/configuration";
const PRODUCTION_CONFIG_FOLDER_NAME: &str = "configuration_verifier";
const CARGO_MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";
const DEFAULT_APP_LOCAL_BASE_FILENAME: &str = "base";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct AppConfig {
    #[serde(rename = "http_server_ip")]
    pub ip: String,
    #[serde(rename = "http_server_port")]
    pub port: u16,
    #[serde(rename = "network")]
    pub network: Network,
}

// FIXME!!! This is super bad hardcode, delete these as soon as we have proper indexer logic
pub fn construct_hardcoded_callback_url(app_config: &AppConfig) -> Url {
    Url::parse(&format!(
        "http://{}:{}/api/btc-indexer/notify-runes-deposit",
        app_config.ip, app_config.port
    ))
    .unwrap()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct BtcIndexerConfig {
    pub address: Url,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SparkBalanceCheckerConfig {
    pub address: Url,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SignerConfig {
    pub identifier: u16,
    pub threshold: u16,
    pub total_participants: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct GatewayConfig {
    pub address: Url,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct DatabaseConfig {
    pub url: String,
}

/// Struct used for initialization of different kinds of configurations
///
/// Example of using local configuration:
/// ```rust
/// use global_utils::config_variant::ConfigVariant;
/// use verifier_config_parser::config::ServerConfig;
/// let config = ServerConfig::init_config(ConfigVariant::Local);
/// assert!(config.is_ok())
/// ```
// Example of using production configuration:
// ```
// use global_utils::config_variant::ConfigVariant;
// use verifier_config_parser::config::ServerConfig;
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
    #[serde(rename = "frost_signer_config")]
    pub frost_signer: SignerConfig,
    #[serde(rename = "gateway_config")]
    pub gateway: GatewayConfig,
    #[serde(rename = "database_config")]
    pub database: DatabaseConfig,
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
