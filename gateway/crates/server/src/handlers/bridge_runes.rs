use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::{Address, OutPoint, Txid};
use gateway_deposit_verification::types::VerifyRunesDepositRequest;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::instrument;

#[derive(Deserialize, Debug)]
pub struct BridgeRunesSparkRequest {
    pub btc_address: String,
    pub bridge_address: String,
    pub txid: Txid,
    pub vout: u32,
}

#[instrument(level = "info", skip(request, state), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<BridgeRunesSparkRequest>,
) -> Result<Json<()>, GatewayError> {
    let btc_address = Address::from_str(&request.btc_address)
        .map_err(|e| GatewayError::InvalidData(format!("Failed to parse btc address: {e}")))?
        .require_network(state.network)
        .map_err(|e| GatewayError::InvalidData(format!("Failed to parse btc address: {e}")))?;

    let verify_runes_deposit_request = VerifyRunesDepositRequest {
        btc_address: request.btc_address,
        bridge_address: request.bridge_address,
        out_point: OutPoint::new(request.txid, request.vout),
    };

    let _ = state
        .deposit_verification_aggregator
        .verify_runes_deposit(verify_runes_deposit_request)
        .await
        .map_err(|e| GatewayError::DepositVerificationError(format!("Failed to verify runes deposit: {}", e)))?;

    Ok(Json(()))
}
