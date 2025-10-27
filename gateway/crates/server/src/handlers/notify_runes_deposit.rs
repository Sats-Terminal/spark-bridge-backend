use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::OutPoint;
use gateway_deposit_verification::types::NotifyRunesDepositRequest;
use gateway_local_db_store::schemas::deposit_address::DepositStatus;
use serde::Deserialize;
use tracing::instrument;

#[derive(Deserialize, Debug)]
pub struct VerifierNotifyRunesDepositRequest {
    pub verifier_id: u16,
    pub outpoint: OutPoint,
    pub sats_fee_amount: u64,
    pub status: DepositStatus,
}

impl From<VerifierNotifyRunesDepositRequest> for NotifyRunesDepositRequest {
    fn from(value: VerifierNotifyRunesDepositRequest) -> Self {
        NotifyRunesDepositRequest {
            verifier_id: value.verifier_id,
            outpoint: value.outpoint,
            sats_fee_amount: value.sats_fee_amount,
            status: value.status,
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
