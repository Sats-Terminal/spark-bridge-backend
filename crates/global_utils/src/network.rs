use crate::env_parser::{EnvParser, EnvParserError};
use bitcoin::Network;
use std::net::ToSocketAddrs;
use std::str::FromStr;
use url::Url;

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

#[inline]
pub fn convert_to_http_url(
    addr: impl ToSocketAddrs,
    endpoint: Option<impl AsRef<str>>,
) -> Result<Url, url::ParseError> {
    Url::from_str(&format!(
        "http://{}{}",
        addr.to_socket_addrs().unwrap().next().unwrap(),
        endpoint.as_ref().map(|x| x.as_ref()).unwrap_or("")
    ))
}
