use thiserror::Error;
use persistent_storage::error::DbError;

#[derive(Error, Debug)]
pub enum BtcIndexerServerError {
    #[error("Decode error: {0}")]
    DecodeError(String),
    #[error("Btc indexer local db storage error: {0}")]
    DbError(#[from] DbError),
    #[error("Validation error: {0}")]
    ValidationError(String),
}
