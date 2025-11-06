use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::{OutPoint, Txid};
use gateway_deposit_verification::types::{FeePayment, VerifyRunesDepositRequest};
use global_utils::conversion::decode_address;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::instrument;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct BridgeRunesSparkRequest {
    pub btc_address: String,
    pub bridge_address: String,
    pub txid: String,
    pub vout: u32,
    pub fee_payment: FeePayment,
}

#[derive(Serialize, Debug)]
pub struct BridgeRunesSparkResponse {
    pub request_id: Uuid,
}

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<BridgeRunesSparkRequest>,
) -> Result<Json<BridgeRunesSparkResponse>, GatewayError> {
    let request_btc_address = request.btc_address.clone();
    let request_id = Uuid::new_v4();
    tracing::info!(
        "Handling bridge runes request with btc address: {:?}",
        request_btc_address
    );

    let btc_address = decode_address(&request.btc_address, state.network)
        .map_err(|e| GatewayError::InvalidData(format!("Failed to parse btc address: {e}")))?;

    let txid =
        Txid::from_str(&request.txid).map_err(|e| GatewayError::InvalidData(format!("Failed to parse txid: {e}")))?;

    let verify_runes_deposit_request = VerifyRunesDepositRequest {
        request_id,
        btc_address,
        bridge_address: request.bridge_address,
        outpoint: OutPoint::new(txid, request.vout),
        fee_payment: request.fee_payment,
    };

    state
        .deposit_verification_aggregator
        .verify_runes_deposit(verify_runes_deposit_request)
        .await
        .map_err(|e| GatewayError::DepositVerificationError(format!("Failed to verify runes deposit: {}", e)))?;

    tracing::info!(
        "Bridge runes request handled request with btc address: {:?}",
        request_btc_address
    );

    Ok(Json(BridgeRunesSparkResponse { request_id }))
}
