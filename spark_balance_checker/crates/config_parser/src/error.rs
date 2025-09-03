use config::ConfigError;
use std::{io, net::AddrParseError};
use thiserror::Error;

pub type Result<T> = core::result::Result<T, ConfigParserError>;

#[derive(Debug, Error)]
pub enum ConfigParserError {
    #[error("Failed to convert SocketAddr, err: {0}")]
    SocketConversionError(#[from] AddrParseError),
    #[error("Failed to parse address, error: {0}")]
    ConfigMergingError(#[from] ConfigError),
    #[error("Occurred custom error: {0}")]
    Custom(String),
    #[error("Failed to open file, err: {err}, path: {path}")]
    FailedToOpenFile { err: io::Error, path: String },
}
