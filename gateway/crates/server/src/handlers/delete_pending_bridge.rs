use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::{Path, State};
use gateway_local_db_store::schemas::deposit_address::InnerAddress;
use global_utils::conversion::decode_address;
use tracing::instrument;

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(State(state): State<AppState>, Path(btc_address): Path<String>) -> Result<Json<()>, GatewayError> {
    let address = decode_address(&btc_address, state.network)
        .map_err(|e| GatewayError::InvalidData(format!("Invalid btc address: {e}")))?;

    state
        .deposit_verification_aggregator
        .delete_pending_bridge_by_address(address)
        .await
        .map_err(|e| GatewayError::DepositVerificationError(e.to_string()))?;

    Ok(Json(()))
}
