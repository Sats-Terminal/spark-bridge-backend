use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use bitcoin::secp256k1;
use thiserror::Error;
use tracing;

#[derive(Error, Debug, Clone)]
pub enum GatewayError {
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Flow processor error: {0}")]
    FlowProcessorError(String),
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Occurred error with Elliptic Curve swcp256k1, err: {0}")]
    Secp256k1Error(#[from] secp256k1::Error),
    #[error("Failed to parse url, err: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("Deposit verification error: {0}")]
    DepositVerificationError(String),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        tracing::error!("Gateway error: {:?}", self.clone());
        match self {
            GatewayError::BadRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
            GatewayError::FlowProcessorError(message) => (StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
            GatewayError::InvalidData(message) => (StatusCode::BAD_REQUEST, message).into_response(),
            GatewayError::Secp256k1Error(message) => {
                (StatusCode::INTERNAL_SERVER_ERROR, message.to_string()).into_response()
            }
            GatewayError::UrlParseError(message) => {
                (StatusCode::INTERNAL_SERVER_ERROR, message.to_string()).into_response()
            }
            GatewayError::DepositVerificationError(message) => {
                (StatusCode::INTERNAL_SERVER_ERROR, message).into_response()
            }
        }
    }
}
