use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use spark_client::SparkClientError;
use thiserror::Error;

impl From<SparkClientError> for ServerError {
    fn from(error: SparkClientError) -> Self {
        match error {
            SparkClientError::ConnectionError(message) => {
                ServerError::ConnectionError(format!("Spark client connection error: {}", message))
            }
            SparkClientError::DecodeError(message) => {
                ServerError::DecodeError(format!("Spark client decode error: {}", message))
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("decode error: {0}")]
    DecodeError(String),
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ServerError::ConnectionError(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
            ServerError::DecodeError(message) => (StatusCode::BAD_REQUEST, message),
            ServerError::InvalidData(message) => (StatusCode::BAD_REQUEST, message),
        };

        (status, error_message).into_response()
    }
}
