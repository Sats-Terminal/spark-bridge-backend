use bitcoin::secp256k1;
use persistent_storage::error::DbError;
use spark_address::SparkAddressError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlowProcessorError {
    #[error("Channel closed error: {0}")]
    ChannelClosedError(String),
    #[error("Invalid response type: {0}")]
    InvalidResponseType(String),
    #[error("Frost aggregator error: {0}")]
    FrostAggregatorError(String),
    #[error("Invalid data error: {0}")]
    InvalidDataError(String),
    #[error("Database error: {0}")]
    DbError(#[from] DbError),
    #[error("Elliptic curve (secp256k1) error: {0}")]
    Secp256k1Error(#[from] secp256k1::Error),
    #[error("Failed conversion to TweakedPubkey error: {0}")]
    TweakingConversionError(String),
    #[error("Unfinished dkg state: {0}")]
    UnfinishedDkgState(String),
    #[error("Spark service error: {0}")]
    SparkServiceError(String),
    #[error("Rune transfer error: {0}")]
    RuneTransferError(String),
    #[error("Spark address error: {0}")]
    SparkAddressError(#[from] SparkAddressError),
}
