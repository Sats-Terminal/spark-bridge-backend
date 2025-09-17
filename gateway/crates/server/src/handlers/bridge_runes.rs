use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use tracing::instrument;
use serde::{Deserialize, Serialize};
use bitcoin::{Txid, Address};
use std::str::FromStr;

#[derive(Deserialize, Debug)]
pub struct BridgeRunesSparkRequest {
    pub btc_address: String,
    pub txid: Txid,
}

#[derive(Serialize, Debug)]
pub struct BridgeRunesSparkResponse {
    pub message: String,
}

#[instrument(level = "info", skip(request, state), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<BridgeRunesSparkRequest>,
) -> Result<Json<BridgeRunesSparkResponse>, GatewayError> {
    let btc_address = Address::from_str(&request.btc_address)
        .map_err(|e| GatewayError::InvalidData(format!("Failed to parse btc address: {e}")))?
        .require_network(state.network)
        .map_err(|e| GatewayError::InvalidData(format!("Failed to parse btc address: {e}")))?;

    let _ = state.deposit_verification_aggregator.verify_runes_deposit(btc_address, request.txid)
        .await
        .map_err(|e| GatewayError::DepositVerificationError(format!("Failed to verify runes deposit: {}", e)))?;

    Ok(Json(BridgeRunesSparkResponse {
        message: "success".to_string(),
    }))
}
