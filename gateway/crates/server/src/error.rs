use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Bad request: {0}")]
    BadRequest(String),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        match self {
            GatewayError::BadRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
        }
    }
}
