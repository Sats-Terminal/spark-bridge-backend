use crate::error::GatewayError;
use crate::init::AppState;
use axum::{Json, extract::State};
use gateway_flow_processor::flow_sender::TypedMessageSender;
use gateway_flow_processor::types::IssueSparkDepositAddressRequest;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::instrument;

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
#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<GetSparkDepositAddressRequest>,
) -> Result<Json<GetSparkDepositAddressResponse>, GatewayError> {
    let request_user_public_key = request.user_public_key.clone();
    tracing::info!(
        "Handling get spark deposit address request with user public key: {:?}",
        request_user_public_key
    );

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

    tracing::info!(
        "Get spark deposit address request handled request with user public key: {:?}",
        request_user_public_key
    );

    return Ok(Json(GetSparkDepositAddressResponse {
        address: response.addr_to_replenish,
    }));
}
