use thiserror::Error;

#[derive(Error, Debug)]
pub enum SparkClientError {
    #[error("Spark address error: {0}")]
    SparkAddressError(#[from] SparkAddressError),
    #[error("Failed to connect: {0}")]
    ConnectionError(String),
    #[error("Config error: {0}")]
    ConfigError(String),
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
    #[error("Failed to decode: {0}")]
    DecodeError(String),
}

#[derive(Error, Debug)]
pub enum SparkAddressError {
    #[error("Failed to decode: {0}")]
    DecodeError(String),
    #[error("Failed to encode: {0}")]
    EncodeError(String),
}
