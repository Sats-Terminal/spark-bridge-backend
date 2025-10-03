use persistent_storage::error::DbError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DepositVerificationError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Flow processor error: {0}")]
    FlowProcessorError(String),
    #[error("Http error: {0}")]
    HttpError(String),
    #[error("Database error: {0}")]
    DbError(#[from] DbError),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid data: {0}")]
    InvalidDataError(String),
    #[error("Failed to check status of verifier id: {id}, msg: [{msg}]")]
    FailedToCheckStatusOfVerifier { id: u16, msg: String },
    #[error("Failed to check health of deposit verifier, msg: [{msg}]")]
    FailedToCheckHealthOfDepositVerifier { id: u16, msg: String },
}
