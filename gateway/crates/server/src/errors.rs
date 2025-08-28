use axum::{
    response::{
        IntoResponse,
        Response,
    },
    http::StatusCode,

};

pub enum GatewayError {
    BadRequest(String),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        match self {
            GatewayError::BadRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
        }
    }
}