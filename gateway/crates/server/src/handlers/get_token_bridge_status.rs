use crate::error::GatewayError;
use crate::init::AppState;
use axum::{Json, extract::State};
use gateway_local_db_store::schemas::session_storage::{RequestType, SessionStatus, SessionUuid};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

#[derive(Deserialize, Debug)]
pub struct GetTokenBridgeStatusRequest {
    pub session_uuid: SessionUuid,
}

#[derive(Serialize, Debug)]
pub struct GetTokenBridgeStatusResponse {
    pub req_type: RequestType,
    pub status: SessionStatus,
}

#[instrument(level = "info", skip(_state, request), fields(request = ?request), ret)]
pub async fn handle(
    State(_state): State<AppState>,
    Json(request): Json<GetTokenBridgeStatusRequest>,
) -> Result<Json<GetTokenBridgeStatusResponse>, GatewayError> {
    debug!("[handler-btc-addr-issuing] Handling request: {request:?}");

    // TODO: implement with real values
    Ok(Json(GetTokenBridgeStatusResponse {
        req_type: RequestType::GetRunesDepositAddress,
        status: SessionStatus::Completed,
    }))
}
