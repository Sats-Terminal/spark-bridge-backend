use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use persistent_storage::error::DbError;
use thiserror::Error;
use tracing;

#[derive(Error, Debug)]
pub enum BtcIndexerServerError {
    #[error("Decode error: {0}")]
    DecodeError(String),
    #[error("Btc indexer local db storage error: {0}")]
    DbError(#[from] DbError),
    #[error("Validation error: {0}")]
    ValidationError(String),
}

impl IntoResponse for BtcIndexerServerError {
    fn into_response(self) -> Response {
        tracing::error!("Btc indexer server error: {:?}", self);
        match self {
            BtcIndexerServerError::DecodeError(message) => (StatusCode::BAD_REQUEST, message).into_response(),
            BtcIndexerServerError::DbError(message) => {
                (StatusCode::INTERNAL_SERVER_ERROR, message.to_string()).into_response()
            }
            BtcIndexerServerError::ValidationError(message) => (StatusCode::BAD_REQUEST, message).into_response(),
        }
    }
}
