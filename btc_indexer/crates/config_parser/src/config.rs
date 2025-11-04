use std::{fmt::Debug, net::SocketAddr, str::FromStr};

use crate::error::ConfigParserError;
use bitcoin::Network;
use bitcoincore_rpc::{Auth, bitcoin};
use config::{Config, Environment};
use global_utils::common_types::Url;
use global_utils::config_variant::ConfigVariant;
use global_utils::env_parser;
use global_utils::network::NetworkConfig;
use serde::{Deserialize, Serialize};
use std::env::{self, VarError};
use tracing::{debug, instrument, trace};

const CONFIG_FOLDER_NAME: &str = "../../infrastructure/configuration";
const PRODUCTION_CONFIG_FOLDER_NAME: &str = "configuration_indexer";
const CARGO_MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";
const DEFAULT_APP_LOCAL_BASE_FILENAME: &str = "base";
pub const BITCOIN_RPC_HOST: &str = "BITCOIN_RPC_HOST";
pub const BITCOIN_RPC_PORT: &str = "BITCOIN_RPC_PORT";
pub const BITCOIN_RPC_USERNAME: &str = "BITCOIN_RPC_USERNAME";
pub const BITCOIN_RPC_PASSWORD: &str = "BITCOIN_RPC_PASSWORD";
pub const TITAN_URL: &str = "TITAN_URL";
pub const MAESTRO_API_URL: &str = "MAESTRO_API_URL";
pub const MAESTRO_API_KEY: &str = "MAESTRO_API_KEY";

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
    pub app_config: AppConfig,
    #[serde(rename(deserialize = "btc_indexer"))]
    pub btc_indexer_config: BtcIndexerParams,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub http_server_ip: String,
    pub http_server_port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BtcRpcCredentials {
    pub url: String,
    pub network: Network,
    pub name: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct BtcIndexerParams {
    pub update_interval_millis: u64,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TitanConfig {
    pub url: Url,
}

impl TitanConfig {
    pub fn new() -> crate::error::Result<Self> {
        let url_to_parse = env_parser::obtain_env_value(TITAN_URL)?;
        Ok(Self {
            url: Url::from_str(&url_to_parse).map_err(|e| ConfigParserError::ParseUrlError {
                url: url_to_parse,
                err: e,
            })?,
        })
    }

    pub fn maybe_new() -> crate::error::Result<Option<Self>> {
        match env::var(TITAN_URL) {
            Ok(url_to_parse) => {
                let url = Url::from_str(&url_to_parse).map_err(|err| ConfigParserError::ParseUrlError {
                    url: url_to_parse.clone(),
                    err,
                })?;
                Ok(Some(Self { url }))
            }
            Err(VarError::NotPresent) => Ok(None),
            Err(err) => Err(ConfigParserError::ConfigEnvParseError(
                env_parser::EnvParserError::ConfigEnvParseError {
                    missing_var_name: TITAN_URL.to_string(),
                    err,
                },
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MaestroConfig {
    pub base_url: Url,
    pub api_key: String,
}

impl MaestroConfig {
    pub fn maybe_from_env() -> crate::error::Result<Option<Self>> {
        match env::var(MAESTRO_API_URL) {
            Ok(url_to_parse) => {
                let base_url = Url::from_str(&url_to_parse).map_err(|err| ConfigParserError::ParseUrlError {
                    url: url_to_parse.clone(),
                    err,
                })?;
                let api_key = env_parser::obtain_env_value(MAESTRO_API_KEY)?;
                Ok(Some(Self { base_url, api_key }))
            }
            Err(VarError::NotPresent) => Ok(None),
            Err(err) => Err(ConfigParserError::ConfigEnvParseError(
                env_parser::EnvParserError::ConfigEnvParseError {
                    missing_var_name: MAESTRO_API_URL.to_string(),
                    err,
                },
            )),
        }
    }
}

impl ServerConfig {
    #[instrument(level = "debug", ret)]
    pub fn init_config(config_variant: ConfigVariant) -> crate::error::Result<Self> {
        trace!("Initializing, {config_variant}...");
        let format_name = |folder_path: &str, config_folder_name: &str, filename: &str| -> String {
            format!("{folder_path}{config_folder_name}/{}.toml", filename)
        };
        let _ = dotenvy::dotenv().ok();
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

pub fn get_cargo_manifest_dir() -> String {
    std::env::var(CARGO_MANIFEST_DIR).unwrap()
}

impl BtcRpcCredentials {
    pub fn get_btc_creds(&self) -> Auth {
        if self.name.is_empty() && self.password.is_empty() {
            Auth::None
        } else {
            Auth::UserPass(self.name.clone(), self.password.clone())
        }
    }

    #[instrument(level = "trace", ret)]
    pub fn new() -> crate::error::Result<Self> {
        Ok(Self {
            url: env_parser::obtain_env_value(BITCOIN_RPC_HOST)?,
            network: NetworkConfig::from_env()
                .map_err(|e| ConfigParserError::ParseNetworkError(e.to_string()))?
                .network,
            name: env_parser::obtain_env_value(BITCOIN_RPC_USERNAME)?,
            password: env_parser::obtain_env_value(BITCOIN_RPC_PASSWORD)?,
        })
    }
}
