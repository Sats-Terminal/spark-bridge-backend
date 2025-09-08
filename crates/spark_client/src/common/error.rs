use thiserror::Error;

#[derive(Error, Debug)]
pub enum SparkClientError {
    #[error("Failed to decode: {0}")]
    DecodeError(String),
    #[error("Failed to connect: {0}")]
    ConnectionError(String),
    #[error("Config error: {0}")]
    ConfigError(String),
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
}
