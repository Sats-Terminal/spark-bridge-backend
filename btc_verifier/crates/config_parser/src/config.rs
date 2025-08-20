use std::{
    fmt::Debug,
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use bitcoincore_rpc::{Auth, bitcoin::Network};
use config::{Config, Environment};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::{config::env_parser::EnvParser, error::ConfigParserError};

const CONFIG_FOLDER_NAME: &str = "configuration";
const CARGO_MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";
pub const APP_CONFIGURATION_NAME: &str = "APP_ENVIRONMENT";
pub const SSH_PRIVATE_KEY_PATH: &str = "SSH_PRIVATE_KEY_PATH";
pub const DEFAULT_APP_PRODUCTION_CONFIG_NAME: &str = "production";
const DEFAULT_APP_LOCAL_BASE_FILENAME: &str = "base.toml";
pub const DEFAULT_APP_LOCAL_CONFIG_NAME: &str = "local";
pub const POSTGRES_TESTING_URL_ENV_NAME: &str = "DATABASE_URL_TESTING";
pub const POSTGRES_URL_ENV_NAME: &str = "DATABASE_URL";
pub const BITCOIN_NETWORK: &str = "BITCOIN_NETWORK";
pub const BITCOIN_RPC_HOST: &str = "BITCOIN_RPC_HOST";
pub const BITCOIN_RPC_PORT: &str = "BITCOIN_RPC_PORT";
pub const BITCOIN_RPC_USERNAME: &str = "BITCOIN_RPC_USERNAME";
pub const BITCOIN_RPC_PASSWORD: &str = "BITCOIN_RPC_PASSWORD";

/// Struct used for initialization of different kinds of configurations
///
/// Example of using local configuration:
/// ```rust
/// use config_parser::config::{ConfigVariant, DEFAULT_APP_LOCAL_CONFIG_NAME, ServerConfig};
/// let config = ServerConfig::init_config(ConfigVariant::Local);
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostgresDbTestingCredentials {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostgresDbCredentials {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BtcRpcCredentials {
    pub url: SocketAddr,
    pub network: Network,
    pub name: String,
    pub password: String,
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
                format!("{}/../", get_cargo_manifest_dir())
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

mod env_parser {
    use crate::error::ConfigParserError;

    pub trait EnvParser {
        const ENV_NAME: &'static str;
        fn obtain_env_value() -> crate::error::Result<String> {
            obtain_env_value(Self::ENV_NAME)
        }
    }

    pub fn obtain_env_value(name: impl AsRef<str>) -> crate::error::Result<String> {
        Ok(
            std::env::var(name.as_ref()).map_err(|err| ConfigParserError::ConfigEnvParseError {
                missing_var_name: name.as_ref().to_string(),
                err,
            })?,
        )
    }
}

impl EnvParser for PostgresDbTestingCredentials {
    const ENV_NAME: &'static str = POSTGRES_TESTING_URL_ENV_NAME;
}

impl EnvParser for PostgresDbCredentials {
    const ENV_NAME: &'static str = POSTGRES_URL_ENV_NAME;
}

impl PostgresDbTestingCredentials {
    pub fn new() -> crate::error::Result<Self> {
        Ok(Self {
            url: Self::obtain_env_value()?,
        })
    }
}

impl PostgresDbCredentials {
    pub fn new() -> crate::error::Result<Self> {
        Ok(Self {
            url: Self::obtain_env_value()?,
        })
    }
}

impl BtcRpcCredentials {
    pub fn get_btc_creds(&self) -> Auth {
        if self.name.is_empty() && self.password.is_empty() {
            Auth::None
        } else {
            Auth::UserPass(self.name.clone(), self.password.clone())
        }
    }

    pub fn new() -> crate::error::Result<Self> {
        Ok(Self {
            url: SocketAddr::new(
                IpAddr::from_str(&env_parser::obtain_env_value(BITCOIN_RPC_HOST)?)?,
                u16::from_str(&env_parser::obtain_env_value(BITCOIN_RPC_PORT)?).map_err(|e| {
                    ConfigParserError::ParseIntError {
                        var_name: BITCOIN_RPC_PORT.to_string(),
                        err: e,
                    }
                })?,
            ),
            network: Network::from_str(&env_parser::obtain_env_value(BITCOIN_NETWORK)?)?,
            name: env_parser::obtain_env_value(BITCOIN_RPC_USERNAME)?,
            password: env_parser::obtain_env_value(BITCOIN_RPC_PASSWORD)?,
        })
    }
}
