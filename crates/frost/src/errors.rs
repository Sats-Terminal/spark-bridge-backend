use persistent_storage::error::DbError;
use thiserror::Error;
use uuid::Uuid;

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
    #[error("DkgShareId already exists")]
    DkgShareIdAlreadyExists(Uuid),
    #[error("DkgShareId not found")]
    DkgShareIdNotFound(Uuid),
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
    #[error("DkgShareId already exists, id: {0}")]
    DkgShareIdAlreadyExists(Uuid),
    #[error("DkgShareId not found, id: {0}")]
    DkgShareIdNotFound(Uuid),
}
