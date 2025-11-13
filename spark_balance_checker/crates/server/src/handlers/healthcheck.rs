use axum::{Json, extract::State};
use global_utils::common_resp::Empty;
use spark_protos::spark_token::QueryTokenTransactionsRequest;
use tonic_health::pb::health_check_response::ServingStatus;

use crate::{error::ServerError, init::AppState};

const EXPECTED_SPARK_OPERATOR_STATUS: ServingStatus = ServingStatus::Serving;

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
            issuer_public_keys: vec![],
            token_identifiers: vec![],
            token_transaction_hashes: vec![],
            limit: 1,
            offset: 0,
            order: 0,
        })
        .await
        .map_err(|e| ServerError::HealthCheckError {
            msg: "Failed to query 1 transaction from Spark".to_string(),
            err: e,
        })?;
    let obtained_so_status =
        state
            .client
            .check_spark_operator_service()
            .await
            .map_err(|e| ServerError::HealthCheckError {
                msg: "Failed to check spark operator service status".to_string(),
                err: e,
            })?;
    if EXPECTED_SPARK_OPERATOR_STATUS != obtained_so_status {
        return Err(ServerError::IncorrectHealthCheckStatus {
            msg: format!(
                "Not healthy status of spark operator, got: {:?}, has to be: {:?}",
                obtained_so_status, EXPECTED_SPARK_OPERATOR_STATUS
            ),
        });
    }
    Ok(Json(Empty {}))
}
