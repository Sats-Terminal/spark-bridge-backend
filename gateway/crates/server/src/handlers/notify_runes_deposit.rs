use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use bitcoin::Txid;
use gateway_local_db_store::schemas::deposit_address::DepositStatus;

#[derive(Serialize, Deserialize, Debug)]
pub struct NotifyRunesAddressRequest {
    pub verifier_id: u16,
    pub txid: Txid,
    pub verifier_response: DepositStatus,
}

#[instrument(level = "info", skip(request, state), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<NotifyRunesAddressRequest>,
) -> Result<Json<()>, GatewayError> {
    // TODO: This request should spawn task and immediately return Json(())
    let _ = state.deposit_verification_aggregator.notify_runes_deposit(request.verifier_id, request.txid, request.verifier_response).await;

    Ok(Json(()))
}
