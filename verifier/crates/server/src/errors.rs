use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VerifierError {
    #[error("Internal server error: {0}")]
    BadRequest(String),
}

impl IntoResponse for VerifierError {
    fn into_response(self) -> Response {
        match self {
            VerifierError::BadRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
        }
    }
}
