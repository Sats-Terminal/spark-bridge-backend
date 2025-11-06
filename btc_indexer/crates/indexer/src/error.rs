use thiserror::Error;
use btc_indexer_client::error::BtcIndexerClientError;
use persistent_storage::error::DbError;

#[derive(Error, Debug)]
pub enum IndexerError {
    #[error("Callback client error: {0}")]
    CallbackClientError(String),
    #[error("Btc indexer client error: {0}")]
    BtcIndexerClientError(#[from] BtcIndexerClientError),
    #[error("Db error: {0}")]
    DbError(#[from] DbError),
    #[error("Validation metadata not found")]
    InvalidData(String),
}
