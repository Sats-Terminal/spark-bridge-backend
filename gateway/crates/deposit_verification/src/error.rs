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
    #[error("Failed to check status of verifier id: {id}, msg: [{msg}]")]
    FailedToCheckStatusOfVerifier { id: u16, msg: String },
}
