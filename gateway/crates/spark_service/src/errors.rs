use spark_address::SparkAddressError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SparkServiceError {
    #[error("Spark client error: {0}")]
    SparkClientError(String),
    #[error("Frost aggregator error: {0}")]
    FrostAggregatorError(String),
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Hash error: {0}")]
    HashError(String),
    #[error("Decode error: {0}")]
    DecodeError(String),
    #[error("Spark address error: {0}")]
    SparkAddressError(#[from] SparkAddressError),
}
