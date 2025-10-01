use thiserror::Error;
use persistent_storage::error::DbError;

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
}
