use persistent_storage::error::DatabaseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SignerError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Invalid user state: {0}")]
    InvalidUserState(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Musig already exists")]
    MusigAlreadyExists(String),
    #[error("Musig not found")]
    MusigNotFound(String),
}

#[derive(Error, Debug)]
pub enum AggregatorError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Invalid user state: {0}")]
    InvalidUserState(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Signer error: {0}")]
    SignerError(#[from] SignerError),
    #[error("HTTP error: {0}")]
    HttpError(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Musig already exists")]
    MusigAlreadyExists(String),
    #[error("Musig not found")]
    MusigNotFound(String),
}
