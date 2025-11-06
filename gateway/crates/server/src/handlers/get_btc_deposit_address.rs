use crate::error::GatewayError;
use crate::init::AppState;
use axum::{Json, extract::State};
use gateway_flow_processor::flow_sender::TypedMessageSender;
use gateway_flow_processor::types::IssueBtcDepositAddressRequest;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;
use std::str::FromStr;
use gateway_local_db_store::schemas::user_identifier::UserId;

#[derive(Deserialize, Debug)]
pub struct GetBtcDepositAddressRequest {
    pub user_id: String,
    pub rune_id: String,
    pub amount: u64,
}

#[derive(Serialize, Debug)]
pub struct GetBtcDepositAddressResponse {
    pub address: String,
    pub fee: Option<FeeData>,
}

#[derive(Serialize, Debug)]
pub struct FeeData {
    pub amount: u64,
    pub btc_address: String,
    pub spark_address: String,
}

/// Handles Btc address issuing for replenishment
#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<GetBtcDepositAddressRequest>,
) -> Result<Json<GetBtcDepositAddressResponse>, GatewayError> {
    let request_uuid = request.user_id.clone();
    tracing::info!(
        "Handling get btc deposit address request with user public key: {:?}",
        request_uuid
    );

    let user_id = UserId::from_str(&request.user_id).map_err(|e| GatewayError::InvalidData(format!("Invalid user id: {}", e)))?;

    let possible_response = state
        .flow_sender
        .send(IssueBtcDepositAddressRequest {
            user_id: user_id,
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
        fee: match state.fee_cfg {
            None => None,
            Some(fee_cfg) => Some(FeeData {
                amount: fee_cfg.amount,
                btc_address: fee_cfg.btc_address.clone(),
                spark_address: fee_cfg.spark_address.clone(),
            }),
        }
    }))
}
