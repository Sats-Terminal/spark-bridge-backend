use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::OutPoint;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use verifier_gateway_client::client::{GatewayNotifyRunesDepositRequest, GatewayNotifyRunesDepositStatus};
use verifier_local_db_store::schemas::deposit_address::{DepositAddressStorage, DepositStatus};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum NotifyRequestStatus {
    Confirmed,
    Failed,
}

impl Into<DepositStatus> for NotifyRequestStatus {
    fn into(self) -> DepositStatus {
        match self {
            NotifyRequestStatus::Confirmed => DepositStatus::Confirmed,
            NotifyRequestStatus::Failed => DepositStatus::Failed,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct IndexerNotifyRequest {
    pub outpoint: OutPoint,
    pub request_id: Uuid,
    pub status: NotifyRequestStatus,
    pub sats_amount: Option<u64>,
    pub rune_id: Option<String>,
    pub rune_amount: Option<u128>,
    pub error_details: Option<String>,
}

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<IndexerNotifyRequest>,
) -> Result<Json<()>, VerifierError> {
    tracing::info!("Runes deposit notified for out point: {}", request.outpoint);

    let sats_amount = request.sats_amount.ok_or(VerifierError::Validation("Sats amount is required".to_string()))?;

    let deposit_status: DepositStatus = request.status.clone().into();

    state.storage
        .set_confirmation_status_by_out_point(request.outpoint, deposit_status.clone(), None)
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set confirmation status: {}", e)))?;

    state.storage
        .set_sats_amount_by_out_point(request.outpoint, sats_amount)
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set sats amount: {}", e)))?;

    let notify_status: GatewayNotifyRunesDepositStatus = match request.status {
        NotifyRequestStatus::Confirmed => GatewayNotifyRunesDepositStatus::Confirmed,
        NotifyRequestStatus::Failed => GatewayNotifyRunesDepositStatus::Failed,
    };

    let gateway_request = GatewayNotifyRunesDepositRequest {
        verifier_id: state.server_config.frost_signer.identifier,
        request_id: request.request_id,
        outpoint: request.outpoint,
        sats_amount: sats_amount,
        status: notify_status,
        error_details: request.error_details,
    };

    state
        .gateway_client
        .notify_runes_deposit(gateway_request)
        .await
        .map_err(|e| VerifierError::GatewayClient(format!("Failed to notify runes deposit: {}", e)))?;

    tracing::info!("Runes deposit notified for out point: {}", request.outpoint);

    Ok(Json(()))
}
