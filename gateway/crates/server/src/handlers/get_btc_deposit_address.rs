use crate::error::GatewayError;
use crate::init::AppState;
use axum::{Json, extract::State};
use gateway_flow_processor::flow_sender::TypedMessageSender;
use gateway_flow_processor::types::IssueBtcDepositAddressRequest;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct GetBtcDepositAddressRequest {
    pub user_id: Uuid,
    pub rune_id: String,
    pub amount: u64,
}

#[derive(Serialize, Debug)]
pub struct GetBtcDepositAddressResponse {
    pub address: String,
}

/// Handles Btc address issuing for replenishment
#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<GetBtcDepositAddressRequest>,
) -> Result<Json<GetBtcDepositAddressResponse>, GatewayError> {
    let request_uuid = request.user_id;
    tracing::info!(
        "Handling get btc deposit address request with user public key: {:?}",
        request_uuid
    );

    let possible_response = state
        .flow_sender
        .send(IssueBtcDepositAddressRequest {
            user_id: request.user_id,
            rune_id: request.rune_id,
            amount: request.amount,
        })
        .await
        .map_err(|e| {
            GatewayError::FlowProcessorError(format!("Failed to issue deposit address for replenishment: {e}"))
        })?;

    tracing::info!(
        "Get btc deposit address request handled request with user public key: {:?}",
        request_uuid
    );

    Ok(Json(GetBtcDepositAddressResponse {
        address: possible_response.addr_to_replenish.to_string(),
    }))
}
