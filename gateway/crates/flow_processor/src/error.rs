use crate::types::DkgFlowRequest;
use bitcoin::secp256k1;
use frost::errors::AggregatorError;
use persistent_storage::error::DbError;
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
    #[error("Occurred problem with ussuing btc addr: {0}")]
    BtcAddrIssueError(#[from] BtcAddrIssueErrorEnum),
}

#[derive(Error, Debug)]
pub enum BtcAddrIssueErrorEnum {
    #[error("Unfinished dkg state, please wait for completion. got: {got}, has to be Finalized")]
    UnfinishedDkgState { got: String },
    #[error("No required entry in db for request: {0:?}, while MuSigId exists")]
    NoDepositAddrInfoInDb(DkgFlowRequest),
    #[error("Occurred error on Aggregator, failed to finalize dkg, err: {0}")]
    AggregatorError(#[from] AggregatorError),
    #[error("Database error occurred, err: {0}")]
    DbError(#[from] DbError),
}
