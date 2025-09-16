use persistent_storage::error::DbError;
use reqwest::StatusCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BtcAggregatorError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}
