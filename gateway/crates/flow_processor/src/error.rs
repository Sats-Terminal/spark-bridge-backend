use crate::types::IssueBtcDepositAddressRequest;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bitcoin::secp256k1;
use frost::errors::AggregatorError;
use gateway_local_db_store::schemas::deposit_address::DepositStatus;
use global_utils::api_result_request::ErrorIntoStatusMsgTuple;
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
    #[error("Unfinished dkg state: {0}")]
    UnfinishedDkgState(String),
}
