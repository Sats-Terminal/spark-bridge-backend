use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use gateway_deposit_verification::types::NotifyRunesDepositRequest;
use tracing::instrument;
use bitcoin::OutPoint;
use gateway_local_db_store::schemas::deposit_address::DepositStatus;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct VerifierNotifyRunesDepositRequest {
    pub verifier_id: u16,
    pub out_point: OutPoint,
    pub sats_fee_amount: u64,
    pub status: DepositStatus,
}

impl Into<NotifyRunesDepositRequest> for VerifierNotifyRunesDepositRequest {
    fn into(self) -> NotifyRunesDepositRequest {
        NotifyRunesDepositRequest {
            verifier_id: self.verifier_id,
            out_point: self.out_point,
            sats_fee_amount: self.sats_fee_amount,
            status: self.status,
        }
    }
}

#[instrument(level = "info", skip(request, state), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<VerifierNotifyRunesDepositRequest>,
) -> Result<Json<()>, GatewayError> {
    // TODO: This request should spawn task and immediately return Json(())
    let _ = state
        .deposit_verification_aggregator
        .notify_runes_deposit(request.into())
        .await;

    Ok(Json(()))
}
