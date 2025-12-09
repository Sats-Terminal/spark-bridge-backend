use axum::{Json, extract::State};
use bitcoin::OutPoint;
use gateway_deposit_verification::types::NotifyRunesDepositRequest;
use gateway_local_db_store::schemas::deposit_address::DepositStatus;
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use crate::{error::GatewayError, init::AppState};

#[derive(Deserialize, Debug)]
pub enum NotifyRequestStatus {
    Confirmed,
    Failed,
}

impl From<NotifyRequestStatus> for DepositStatus {
    fn from(val: NotifyRequestStatus) -> Self {
        match val {
            NotifyRequestStatus::Confirmed => DepositStatus::Confirmed,
            NotifyRequestStatus::Failed => DepositStatus::Failed,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct VerifierNotifyRunesDepositRequest {
    pub verifier_id: u16,
    pub request_id: Uuid,
    pub outpoint: OutPoint,
    pub sats_amount: u64,
    pub status: NotifyRequestStatus,
}

impl From<VerifierNotifyRunesDepositRequest> for NotifyRunesDepositRequest {
    fn from(value: VerifierNotifyRunesDepositRequest) -> Self {
        NotifyRunesDepositRequest {
            verifier_id: value.verifier_id,
            request_id: value.request_id,
            outpoint: value.outpoint,
            sats_amount: value.sats_amount,
            status: value.status.into(),
            error_details: None,
        }
    }
}

#[instrument(level = "trace", skip(request, state), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<VerifierNotifyRunesDepositRequest>,
) -> Result<Json<()>, GatewayError> {
    let outpoint = request.outpoint;
    let deposit_request: NotifyRunesDepositRequest = request.into();
    let aggregator = state.deposit_verification_aggregator.clone();

    tracing::info!("Handling notify runes deposit request with out point: {:?}", outpoint);

    tokio::spawn(async move {
        if let Err(err) = aggregator.notify_runes_deposit(deposit_request).await {
            tracing::error!(?outpoint, error = %err, "Failed to process notify_runes_deposit");
        }
    });

    tracing::info!(
        "Notify runes deposit request handled request with out point: {:?}",
        outpoint
    );

    Ok(Json(()))
}
