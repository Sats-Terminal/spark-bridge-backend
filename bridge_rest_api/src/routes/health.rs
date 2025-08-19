use actix_web::{HttpResponseBuilder, Responder, http::StatusCode};
use tracing::{debug, instrument};

/// Handler for the `/health` endpoint returns a boolean value indicating the backend status.
#[utoipa::path(
    get,
    operation_id = "get_health",
    path = "/health",
    tag = "test_functions",
    responses(
        (
            status = 200,
            description = "Backend is successfully started and returns a boolean value",
            body = bool,
            content_type = "application/json",
            example = json!(true)
        ),
    ),
)]
#[instrument(level = "debug", ret)]
pub async fn handle() -> impl Responder {
    debug!("Successfully process 'GET /health'");
    HttpResponseBuilder::new(StatusCode::OK).json(true)
}
