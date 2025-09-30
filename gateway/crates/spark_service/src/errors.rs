use thiserror::Error;
use spark_address::SparkAddressError;
use bitcoin::secp256k1;

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
    #[error("Secp256k1 error: {0}")]
    Secp256k1Error(#[from] secp256k1::Error),
}
