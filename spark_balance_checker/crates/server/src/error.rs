use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use spark_client::common::error::SparkClientError;
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
            SparkClientError::AuthenticationError(message) => {
                ServerError::ConnectionError(format!("Spark client authentication error: {}", message))
            }
            SparkClientError::ConfigError(message) => {
                ServerError::InvalidData(format!("Spark client config error: {}", message))
            }
            SparkClientError::NoAuthSessionFound(message) => {
                ServerError::InvalidData(format!("Spark client no auth session found error: {}", message))
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
    #[error("Failed to check service health, msg: '{msg}', err: {err}")]
    HealthCheckError { msg: String, err: SparkClientError },
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ServerError::ConnectionError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ServerError::DecodeError(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ServerError::InvalidData(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ServerError::HealthCheckError { .. } => (StatusCode::BAD_REQUEST, self.to_string()),
        };

        (status, error_message).into_response()
    }
}
