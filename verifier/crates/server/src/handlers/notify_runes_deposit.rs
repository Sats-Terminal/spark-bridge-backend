use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use tracing::instrument;
use verifier_gateway_client::client::NotifyRunesDepositRequest;
use verifier_local_db_store::schemas::deposit_address::DepositAddressStorage;

#[instrument(level = "debug", skip_all, ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<NotifyRunesDepositRequest>,
) -> Result<Json<()>, VerifierError> {
    // TODO: This request should spawn task and immediately return Json(())
    state
        .storage
        .set_confirmation_status_by_out_point(request.out_point, request.status.clone())
        .await
        .map_err(|e| VerifierError::StorageError(format!("Failed to update confirmation status: {}", e)))?;

    state
        .storage
        .set_sats_fee_amount_by_out_point(request.out_point, request.sats_fee_amount)
        .await
        .map_err(|e| VerifierError::StorageError(format!("Failed to update sats fee amount: {}", e)))?;

    state
        .gateway_client
        .notify_runes_deposit(request)
        .await
        .map_err(|e| VerifierError::GatewayClientError(format!("Failed to notify runes deposit: {}", e)))?;

    Ok(Json(()))
}
