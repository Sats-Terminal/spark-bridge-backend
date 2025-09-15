use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use btc_resp_aggregator::error::BtcTxCheckerError;
use frost::errors::SignerError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VerifierError {
    #[error("Internal server error: {0}")]
    BadRequest(String),
    #[error("Dkg error: {0}")]
    DkgError(#[from] SignerError),
    #[error("Btc checker error: {0}")]
    BtcCheckerError(#[from] BtcTxCheckerError),
}

impl IntoResponse for VerifierError {
    fn into_response(self) -> Response {
        match self {
            VerifierError::BadRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
            VerifierError::DkgError(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
            VerifierError::BtcCheckerError(error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
            }
        }
    }
}
