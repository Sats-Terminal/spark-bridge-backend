use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use tracing::instrument;
use verifier_local_db_store::schemas::deposit_address::{DepositAddressStorage, TxRejectReason, DepositStatus};
use verifier_gateway_client::client::GatewayNotifyRunesDepositRequest;
use bitcoin::OutPoint;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum BtcTxReview {
    Success,
    Failure { reason: TxRejectReason },
}

impl Into<DepositStatus> for BtcTxReview {
    fn into(self) -> DepositStatus {
        match self {
            BtcTxReview::Success => DepositStatus::Confirmed,
            BtcTxReview::Failure { reason } => DepositStatus::Failed(reason),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BtcIndexerNotifyRunesDepositRequest {
    pub out_point: OutPoint,
    pub status: BtcTxReview,
    pub sats_fee_amount: u64,
}

#[instrument(level = "debug", skip_all, ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<BtcIndexerNotifyRunesDepositRequest>,
) -> Result<Json<()>, VerifierError> {
    // TODO: This request should spawn task and immediately return Json(())
    let deposit_status: DepositStatus = request.status.clone().into();
    let gateway_request = GatewayNotifyRunesDepositRequest {
        verifier_id: state.server_config.frost_signer.identifier,
        out_point: request.out_point,
        sats_fee_amount: request.sats_fee_amount,
        status: deposit_status.clone(),
    };

    state.storage.set_confirmation_status_by_out_point(request.out_point, deposit_status)
        .await
        .map_err(|e| VerifierError::StorageError(format!("Failed to update confirmation status: {}", e)))?;

    state.storage.set_sats_fee_amount_by_out_point(request.out_point, request.sats_fee_amount)
        .await
        .map_err(|e| VerifierError::StorageError(format!("Failed to update sats fee amount: {}", e)))?;

    state.gateway_client.notify_runes_deposit(gateway_request).await.map_err(|e| VerifierError::GatewayClientError(format!("Failed to notify runes deposit: {}", e)))?;

    Ok(Json(()))
}
