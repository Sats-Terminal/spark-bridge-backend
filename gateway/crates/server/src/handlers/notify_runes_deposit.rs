use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::OutPoint;
use gateway_deposit_verification::types::NotifyRunesDepositRequest;
use gateway_local_db_store::schemas::deposit_address::DepositStatus;
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
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
    tracing::info!("Handling notify runes deposit request with out point: {:?}", outpoint);

    tokio::spawn(async move {
        let _ = state
            .deposit_verification_aggregator
            .notify_runes_deposit(request.into())
            .await;
    });

    tracing::info!(
        "Notify runes deposit request handled request with out point: {:?}",
        outpoint
    );

    Ok(Json(()))
}
