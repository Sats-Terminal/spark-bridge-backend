use thiserror::Error;

#[derive(Error, Debug)]
pub enum SparkClientError {
    #[error("Failed to decode: {0}")]
    DecodeError(String),
    #[error("Failed to connect: {0}")]
    ConnectionError(String),
}
