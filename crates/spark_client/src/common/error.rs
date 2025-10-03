use thiserror::Error;
use tonic::metadata::errors::InvalidMetadataValue;

#[derive(Error, Debug)]
pub enum SparkClientError {
    #[error("Failed to connect: {0}")]
    ConnectionError(String),
    #[error("Config error: {0}")]
    ConfigError(String),
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
    #[error("Failed to decode: {0}")]
    DecodeError(String),
    #[error("No auth session found")]
    NoAuthSessionFound(String),
    #[error("Invalid metadata for sending request, possible pubkey: '{possible_pubkey}', err: {err}")]
    InvalidMetadataStr {
        possible_pubkey: String,
        err: InvalidMetadataValue,
    },
}
