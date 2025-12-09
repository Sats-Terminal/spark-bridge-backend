use btc_indexer_client::error::BtcIndexerClientError;
use persistent_storage::error::DbError;
use thiserror::Error;

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
