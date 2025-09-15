use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use bitcoin::secp256k1;
use btc_resp_aggregator::error::BtcAggregatorError;
use global_utils::api_result_request::ErrorIntoStatusMsgTuple;
use persistent_storage::error::DbError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PrivateApiError {
    #[error("Invalid response type: {0}")]
    InvalidResponseType(String),
    #[error("Frost aggregator error: {0}")]
    FrostAggregatorError(String),
    #[error("Invalid data error: {0}")]
    InvalidDataError(String),
    #[error("Database error: {0}")]
    DbError(#[from] DbError),
    #[error("Elliptic curve (secp256k1) error: {0}")]
    Secp256k1Error(#[from] secp256k1::Error),
}

#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Flow processor error: {0}")]
    FlowProcessorError(String),
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Occurred error with Elliptic Curve swcp256k1, err: {0}")]
    Secp256k1Error(#[from] secp256k1::Error),
    #[error("Occurred error btc aggregator, err: {0}")]
    BtcAggregatorError(#[from] BtcAggregatorError),
    #[error("Failed to parse url, err: {0}")]
    UrlParseError(#[from] url::ParseError),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        match self {
            GatewayError::BadRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
            GatewayError::FlowProcessorError(message) => (StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
            GatewayError::InvalidData(message) => (StatusCode::BAD_REQUEST, message).into_response(),
            GatewayError::Secp256k1Error(message) => {
                (StatusCode::INTERNAL_SERVER_ERROR, message.to_string()).into_response()
            }
            GatewayError::BtcAggregatorError(message) => {
                (StatusCode::INTERNAL_SERVER_ERROR, message.to_string()).into_response()
            }
            GatewayError::UrlParseError(message) => {
                (StatusCode::INTERNAL_SERVER_ERROR, message.to_string()).into_response()
            }
        }
    }
}

impl IntoResponse for PrivateApiError {
    fn into_response(self) -> Response {
        self.into_status_msg_tuple().into_response()
    }
}

impl ErrorIntoStatusMsgTuple for PrivateApiError {
    fn into_status_msg_tuple(self) -> (StatusCode, String) {
        match self {
            PrivateApiError::InvalidDataError(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            PrivateApiError::InvalidResponseType(msg) => (StatusCode::NOT_FOUND, msg),
            PrivateApiError::FrostAggregatorError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.to_string()),
            PrivateApiError::DbError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            PrivateApiError::Secp256k1Error(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        }
    }
}
