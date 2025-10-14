use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::OutPoint;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use verifier_gateway_client::client::{GatewayNotifyRunesDepositRequest, GatewayDepositStatus};
use verifier_local_db_store::schemas::deposit_address::{DepositAddressStorage, DepositStatus};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum BtcIndexerDepositStatus {
    Confirmed,
    Failed,
}

impl Into<DepositStatus> for BtcIndexerDepositStatus {
    fn into(self) -> DepositStatus {
        match self {
            BtcIndexerDepositStatus::Confirmed => DepositStatus::Confirmed,
            BtcIndexerDepositStatus::Failed => DepositStatus::Failed,
        }
    }
}

impl Into<GatewayDepositStatus> for BtcIndexerDepositStatus {
    fn into(self) -> GatewayDepositStatus {
        match self {
            BtcIndexerDepositStatus::Confirmed => GatewayDepositStatus::Confirmed,
            BtcIndexerDepositStatus::Failed => GatewayDepositStatus::Failed,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct IndexerNotifyRequest {
    pub outpoint: OutPoint,
    pub request_id: Uuid,
    pub deposit_status: BtcIndexerDepositStatus,
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

    state.storage
        .set_confirmation_status_by_out_point(request.outpoint, request.deposit_status.clone().into(), None)
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set confirmation status: {}", e)))?;

    state.storage
        .set_sats_amount_by_out_point(request.outpoint, sats_amount)
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set sats amount: {}", e)))?;

    let gateway_request = GatewayNotifyRunesDepositRequest {
        verifier_id: state.server_config.frost_signer.identifier,
        request_id: request.request_id,
        outpoint: request.outpoint,
        sats_amount: sats_amount,
        status: request.deposit_status.clone().into(),
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
