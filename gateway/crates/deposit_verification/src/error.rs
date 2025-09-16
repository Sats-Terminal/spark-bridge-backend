use persistent_storage::error::DbError;
use reqwest::StatusCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DepositVerificationError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Flow processor error: {0}")]
    FlowProcessorError(String),
    #[error("Http error: {0}")]
    HttpError(String),
}
