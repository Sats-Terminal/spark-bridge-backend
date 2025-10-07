use crate::error::GatewayError;
use crate::init::AppState;
use axum::{Json, extract::State};
use gateway_flow_processor::flow_sender::TypedMessageSender;
use gateway_flow_processor::types::IssueSparkDepositAddressRequest;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct GetSparkDepositAddressRequest {
    pub user_id: Uuid,
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
    let request_uuid = request.user_id.clone();
    tracing::info!(
        "Handling get spark deposit address request with user public key: {:?}",
        request_uuid
    );

    let response = state
        .flow_sender
        .send(IssueSparkDepositAddressRequest {
            user_id: request.user_id,
            rune_id: request.rune_id,
            amount: request.amount,
        })
        .await
        .map_err(|e| GatewayError::FlowProcessorError(format!("Failed to issue deposit address for bridging: {e}")))?;

    tracing::info!(
        "Get spark deposit address request handled request with user public key: {:?}",
        request_uuid
    );

    Ok(Json(GetSparkDepositAddressResponse {
        address: response.addr_to_replenish,
    }))
}
