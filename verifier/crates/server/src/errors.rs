use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use frost::errors::SignerError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VerifierError {
    #[error("Dkg error: {0}")]
    Dkg(#[from] SignerError),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Btc indexer client error: {0}")]
    BtcIndexerClient(String),
    #[error("Spark balance checker client error: {0}")]
    SparkBalanceCheckerClient(String),
    #[error("Gateway client error: {0}")]
    GatewayClient(String),
    #[error("Validation was incorrect: {0}")]
    Validation(String),
    #[error("Healthcheck error: [{0}]")]
    Healthcheck(String),
}

impl IntoResponse for VerifierError {
    fn into_response(self) -> Response {
        match self {
            VerifierError::Dkg(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
            VerifierError::Storage(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
            VerifierError::BtcIndexerClient(error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
            }
            VerifierError::SparkBalanceCheckerClient(error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
            }
            VerifierError::GatewayClient(error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
            }
            VerifierError::Healthcheck(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
            VerifierError::Validation(message) => (StatusCode::BAD_REQUEST, message).into_response(),
        }
    }
}
