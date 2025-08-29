use std::{env::VarError, io, net::IpAddr};

use dns_lookup::lookup_host;
use thiserror::Error;
use tracing::instrument;

#[derive(Debug, Error)]
pub enum EnvParserError {
    #[error("Failed to parse env variable {missing_var_name}, err: {err}, check if it exists and is valid")]
    ConfigEnvParseError { missing_var_name: String, err: VarError },
    #[error("No host in string: {0}")]
    NoHostInString(String),
    #[error("No host in string: {0}")]
    FailedToLookupAddress(#[from] io::Error),
}

pub trait EnvParser {
    const ENV_NAME: &'static str;
    fn obtain_env_value() -> Result<String, EnvParserError> {
        obtain_env_value(Self::ENV_NAME)
    }
}

#[instrument(level = "debug", skip(name), fields(name = name.as_ref()) ret)]
pub fn obtain_env_value(name: impl AsRef<str>) -> Result<String, EnvParserError> {
    std::env::var(name.as_ref()).map_err(|err| EnvParserError::ConfigEnvParseError {
        missing_var_name: name.as_ref().to_string(),
        err,
    })
}

#[instrument(level = "debug", skip(hostname), fields(name = hostname.as_ref()) ret)]
pub fn lookup_ip_addr(hostname: impl AsRef<str>) -> Result<IpAddr, EnvParserError> {
    lookup_host(hostname.as_ref())?
        .next()
        .ok_or(EnvParserError::NoHostInString(hostname.as_ref().to_string()))
}
