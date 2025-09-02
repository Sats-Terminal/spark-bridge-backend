use crate::error::GatewayError;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use gateway_flow_processor::flow_sender::TypedMessageSender;
use gateway_flow_processor::types::DkgFlowRequest;
use axum::debug_handler;

#[derive(Deserialize)]
pub struct GetRunesAddressRequest {
    pub user_public_key: String,
    pub rune_id: String,
}

#[derive(Serialize)]
pub struct GetRunesAddressResponse {
    pub address: String,
}

pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<GetRunesAddressRequest>,
) -> Result<Json<GetRunesAddressResponse>, GatewayError> {
    let response = state.flow_sender.send(DkgFlowRequest {
        user_public_key: request.user_public_key,
    }).await.map_err(|e| GatewayError::FlowProcessorError(e.to_string()))?;

    Ok(Json(GetRunesAddressResponse {
        address: response.public_key,
    }))
}
