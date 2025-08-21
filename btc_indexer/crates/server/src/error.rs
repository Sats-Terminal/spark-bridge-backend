use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Bad Request: {0}")]
    BadRequest(String),
    #[error("Internal Server Error: {0}")]
    InternalError(String),
    #[error("Not Found: {0}")]
    NotFound(String),
}

impl axum::response::IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;
        let (status, msg) = match self {
            ServerError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ServerError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ServerError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        };
        (status, msg).into_response()
    }
}

pub type Result<T> = std::result::Result<T, ServerError>;
