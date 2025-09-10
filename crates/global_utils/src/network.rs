use crate::env_parser::{EnvParser, EnvParserError};
use bitcoin::Network;
use std::str::FromStr;

pub struct NetworkConfig {
    pub network: Network,
}
impl EnvParser for NetworkConfig {
    const ENV_NAME: &'static str = "BITCOIN_NETWORK";
}
impl NetworkConfig {
    /// Reads CONFIG_PATH env
    pub fn from_env() -> Result<Self, EnvParserError> {
        let obtained_value = NetworkConfig::obtain_env_value()?;
        Ok(Self {
            network: Network::from_str(&obtained_value).map_err(|e| EnvParserError::FailedToParseNetwork {
                string: obtained_value,
                err: e,
            })?,
        })
    }
}
