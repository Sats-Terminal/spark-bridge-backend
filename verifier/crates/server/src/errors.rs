use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use frost::errors::SignerError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VerifierError {
    #[error("Internal server error: {0}")]
    BadRequest(String),
    #[error("Dkg error: {0}")]
    DkgError(#[from] SignerError),
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Decode error: {0}")]
    DecodeError(String),
    #[error("Btc indexer client error: {0}")]
    BtcIndexerClientError(String),
    #[error("Spark balance checker client error: {0}")]
    SparkBalanceCheckerClientError(String),
    #[error("Gateway client error: {0}")]
    GatewayClientError(String),
    #[error("Validation was incorrect: {0}")]
    ValidationError(String),
}

impl IntoResponse for VerifierError {
    fn into_response(self) -> Response {
        match self {
            VerifierError::BadRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
            VerifierError::DkgError(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
            VerifierError::StorageError(error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
            }
            VerifierError::DecodeError(error) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
            VerifierError::BtcIndexerClientError(error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
            }
            VerifierError::SparkBalanceCheckerClientError(error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
            }
            VerifierError::GatewayClientError(error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
            }
            VerifierError::ValidationError(message) => (StatusCode::BAD_REQUEST, message).into_response(),
        }
    }
}
