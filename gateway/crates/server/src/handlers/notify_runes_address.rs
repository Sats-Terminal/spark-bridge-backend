use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use global_utils::common_types::{TxIdWrapped, get_uuid};
use global_utils::network::convert_to_http_url;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::instrument;
use bitcoin::Txid;
use gateway_local_db_store::schemas::deposit_address::DepositStatus;

#[derive(Deserialize, Debug)]
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
    state.deposit_verification_aggregator.notify_runes_deposit(request.verifier_id, request.txid, request.verifier_response);

    Ok(Json(()))
}
