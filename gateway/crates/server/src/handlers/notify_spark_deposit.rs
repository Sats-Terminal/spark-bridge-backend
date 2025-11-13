use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use gateway_deposit_verification::types::NotifySparkDepositRequest;
use gateway_local_db_store::schemas::deposit_address::DepositStatus;
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

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
    pub spark_address: String,
    pub status: NotifyRequestStatus,
}

impl From<VerifierNotifyRunesDepositRequest> for NotifySparkDepositRequest {
    fn from(value: VerifierNotifyRunesDepositRequest) -> Self {
        NotifySparkDepositRequest {
            verifier_id: value.verifier_id,
            request_id: value.request_id,
            spark_address: value.spark_address.clone(),
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
    let addr = request.spark_address.clone();
    tracing::info!("Handling notify spark deposit request for addr: {:?}", addr.clone());

    tokio::spawn(async move {
        let _ = state
            .deposit_verification_aggregator
            .notify_spark_deposit(request.into())
            .await;
    });

    tracing::info!("Notify spark deposit request handled request for addr: {:?}", addr);

    Ok(Json(()))
}
