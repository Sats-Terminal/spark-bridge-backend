use crate::error::ServerError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bech32;
use global_utils::common_resp::Empty;

#[utoipa::path(
    post,
    path = "/health",
    request_body = Empty,
    responses(
        (status = 200, description = "Success", body = Empty),
        (status = 400, description = "Bad Request", body = String),
        (status = 500, description = "Internal Server Error", body = String),
    ),
)]
pub async fn handle(State(mut state): State<AppState>) -> Result<Json<Empty>, ServerError> {
    Ok(Json(Empty {}))
}
