use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Flow processor error: {0}")]
    FlowProcessorError(String),
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        match self {
            GatewayError::BadRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
            GatewayError::FlowProcessorError(message) => (StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
            GatewayError::InvalidData(message) => (StatusCode::BAD_REQUEST, message).into_response(),
        }
    }
}
