use crate::types::MusigId;
use persistent_storage::error::DbError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SignerError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Invalid user state: {0}")]
    InvalidUserState(String),
    #[error("Internal error: '{0}'")]
    Internal(String),
    #[error(transparent)]
    DatabaseError(#[from] DbError),
    #[error("Musig already exists, id: {0:?}")]
    MusigAlreadyExists(MusigId),
    #[error("Musig not found, id: {0:?}")]
    MusigNotFound(MusigId),
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
    #[error(transparent)]
    DatabaseError(#[from] DbError),
    #[error("User unique id already exists, id: {0:?}")]
    MusigAlreadyExists(MusigId),
    #[error("Failed to unlock musig, id: {0:?}")]
    FailedToUnlockMusig(MusigId),
    #[error("User unique id not found")]
    MusigNotFound(String),
}
