use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use gateway_deposit_verification::types::VerifySparkDepositRequest;
use gateway_rune_transfer::transfer::PayingTransferInput;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct ExitSparkRequest {
    pub spark_address: String,
    pub exit_address: String,
    pub paying_input: PayingTransferInput,
}

pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<ExitSparkRequest>,
) -> Result<Json<()>, GatewayError> {
    let verify_spark_deposit_request = VerifySparkDepositRequest {
        spark_address: request.spark_address,
        exit_address: request.exit_address,
        paying_input: request.paying_input,
    };

    let _ = state
        .deposit_verification_aggregator
        .verify_spark_deposit(verify_spark_deposit_request)
        .await
        .map_err(|e| GatewayError::DepositVerificationError(format!("Failed to verify spark deposit: {}", e)))?;

    Ok(Json(()))
}
