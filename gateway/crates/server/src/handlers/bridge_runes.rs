use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::{OutPoint, Txid};
use gateway_deposit_verification::types::VerifyRunesDepositRequest;
use global_utils::conversion::decode_address;
use serde::Deserialize;
use std::str::FromStr;
use tracing::instrument;

#[derive(Deserialize, Debug)]
pub struct BridgeRunesSparkRequest {
    pub btc_address: String,
    pub bridge_address: String,
    pub txid: String,
    pub vout: u32,
}

#[instrument(level = "info", skip(request, state), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<BridgeRunesSparkRequest>,
) -> Result<Json<()>, GatewayError> {
    let btc_address = decode_address(&request.btc_address, state.network)
        .map_err(|e| GatewayError::InvalidData(format!("Failed to parse btc address: {e}")))?;

    let txid =
        Txid::from_str(&request.txid).map_err(|e| GatewayError::InvalidData(format!("Failed to parse txid: {e}")))?;

    let verify_runes_deposit_request = VerifyRunesDepositRequest {
        btc_address,
        bridge_address: request.bridge_address,
        out_point: OutPoint::new(txid, request.vout),
    };

    state
        .deposit_verification_aggregator
        .verify_runes_deposit(verify_runes_deposit_request)
        .await
        .map_err(|e| GatewayError::DepositVerificationError(format!("Failed to verify runes deposit: {}", e)))?;

    Ok(Json(()))
}
