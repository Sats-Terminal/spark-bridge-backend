use crate::error::ServerError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use global_utils::common_resp::Empty;
use spark_protos::spark::QueryTokenTransactionsRequest;

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
pub async fn handle(State(state): State<AppState>) -> Result<Json<Empty>, ServerError> {
    state
        .client
        .query_token_transactions(QueryTokenTransactionsRequest {
            output_ids: vec![],
            owner_public_keys: vec![],
            token_public_keys: vec![],
            token_identifiers: vec![],
            token_transaction_hashes: vec![],
            limit: 1,
            offset: 0,
        })
        .await
        .map_err(|e| ServerError::HealthCheckError {
            msg: "Failed to query 1 transaction from Spark".to_string(),
            err: e,
        })?;
    Ok(Json(Empty {}))
}
