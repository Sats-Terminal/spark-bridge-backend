use crate::error::GatewayError;
use crate::init::AppState;
use axum::{Json, extract::State};
use gateway_flow_processor::types::{IssueSparkDepositAddressRequest};
use gateway_flow_processor::flow_sender::TypedMessageSender;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{debug, instrument};

#[derive(Deserialize, Debug)]
pub struct GetSparkDepositAddressRequest {
    pub user_public_key: String,
    pub rune_id: String,
    pub amount: u64,
}

#[derive(Serialize, Debug)]
pub struct GetSparkDepositAddressResponse {
    pub address: String,
}

/// Handles Btc address issuing for replenishment
#[instrument(level = "info", skip(state, request), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<GetSparkDepositAddressRequest>,
) -> Result<Json<GetSparkDepositAddressResponse>, GatewayError> {
    debug!("[handler-btc-addr-issuing] Handling request: {request:?}");
    let response = state
        .flow_sender
        .send(IssueSparkDepositAddressRequest {
            musig_id: frost::types::MusigId::User {
                rune_id: request.rune_id,
                user_public_key: bitcoin::secp256k1::PublicKey::from_str(&request.user_public_key)
                    .map_err(|e| GatewayError::InvalidData(format!("Failed to parse user public key: {e}")))?,
            },
            amount: request.amount,
        })
        .await
        .map_err(|e| GatewayError::FlowProcessorError(format!("Failed to issue deposit address for bridging: {e}")))?;
    
    return Ok(Json(GetSparkDepositAddressResponse {
        address: response.addr_to_replenish,
    }));
}
