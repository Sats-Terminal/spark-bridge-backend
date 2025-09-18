use crate::error::GatewayError;
use crate::init::AppState;
use axum::{Json, extract::State};
use gateway_flow_processor::flow_sender::TypedMessageSender;
use gateway_flow_processor::types::{FlowProcessorResponse, IssueBtcDepositAddressRequest};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{debug, instrument};

#[derive(Deserialize, Debug)]
pub struct GetBtcDepositAddressRequest {
    pub user_public_key: String,
    pub rune_id: String,
    pub amount: u64,
}

#[derive(Serialize, Debug)]
pub struct GetBtcDepositAddressResponse {
    pub address: String,
}

/// Handles Btc address issuing for replenishment
#[instrument(level = "info", skip(state, request), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<GetBtcDepositAddressRequest>,
) -> Result<Json<GetBtcDepositAddressResponse>, GatewayError> {
    debug!("[handler-btc-addr-issuing] Handling request: {request:?}");
    let possible_response = state
        .flow_sender
        .send(IssueBtcDepositAddressRequest {
            musig_id: frost::types::MusigId::User {
                rune_id: request.rune_id,
                user_public_key: bitcoin::secp256k1::PublicKey::from_str(&request.user_public_key)?,
            },
            amount: request.amount,
        })
        .await
        .map_err(|e| {
            GatewayError::FlowProcessorError(format!("Failed to issue deposit address for replenishment: {e}"))
        })?;

    Ok(Json(GetBtcDepositAddressResponse {
        address: possible_response.addr_to_replenish.to_string(),
    }))
}
