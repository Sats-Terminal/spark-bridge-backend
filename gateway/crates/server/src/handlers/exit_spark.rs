use crate::error::GatewayError;
use axum::Json;
use axum::extract::State;
use crate::init::AppState;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ExitSparkRequest {
    pub spark_address: String,
    pub exit_address: String,
}

pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<ExitSparkRequest>,
) -> Result<Json<()>, GatewayError> {
    let _ = state.deposit_verification_aggregator.verify_spark_deposit(request.spark_address, request.exit_address)
        .await
        .map_err(|e| GatewayError::DepositVerificationError(format!("Failed to verify spark deposit: {}", e)))?;

    Ok(Json(()))
}
