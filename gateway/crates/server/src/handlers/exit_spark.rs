use crate::error::GatewayError;
use axum::Json;
use axum::extract::State;
use crate::init::AppState;
use serde::{Deserialize, Serialize};
use gateway_deposit_verification::types::VerifySparkDepositRequest;

#[derive(Deserialize, Serialize)]
pub struct ExitSparkRequest {
    pub spark_address: String,
    pub exit_address: String,
}

pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<ExitSparkRequest>,
) -> Result<Json<()>, GatewayError> {
    let verify_spark_deposit_request = VerifySparkDepositRequest {
        spark_address: request.spark_address,
        exit_address: request.exit_address,
    };

    let _ = state.deposit_verification_aggregator.verify_spark_deposit(verify_spark_deposit_request)
        .await
        .map_err(|e| GatewayError::DepositVerificationError(format!("Failed to verify spark deposit: {}", e)))?;

    Ok(Json(()))
}
