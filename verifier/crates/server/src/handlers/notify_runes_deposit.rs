use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::OutPoint;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use verifier_gateway_client::client::GatewayNotifyRunesDepositRequest;
use verifier_local_db_store::schemas::deposit_address::{DepositAddressStorage, DepositStatus, TxRejectReason};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum BtcTxReview {
    Success,
    Failure { reason: TxRejectReason },
}

impl From<BtcTxReview> for DepositStatus {
    fn from(value: BtcTxReview) -> Self {
        match value {
            BtcTxReview::Success => DepositStatus::Confirmed,
            BtcTxReview::Failure { reason } => DepositStatus::Failed(reason),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BtcIndexerNotifyRunesDepositRequest {
    pub outpoint: OutPoint,
    pub status: BtcTxReview,
    pub sats_amount: u64,
}

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<BtcIndexerNotifyRunesDepositRequest>,
) -> Result<Json<()>, VerifierError> {
    tracing::info!("Runes deposit notified for out point: {}", request.outpoint);

    tokio::spawn(async move {
        _inner_notify(state, request.clone()).await.inspect_err(|err| {
            tracing::error!("Failed to notify runes deposit for req: '{request:?}', err: '{err}'");
        })
    });

    Ok(Json(()))
}

#[instrument(level = "trace", skip(state), ret)]
async fn _inner_notify(state: AppState, request: BtcIndexerNotifyRunesDepositRequest) -> Result<(), VerifierError> {
    tracing::info!("Notifying runes deposit for out point: {}", request.outpoint);

    let deposit_status: DepositStatus = request.status.clone().into();
    let gateway_request = GatewayNotifyRunesDepositRequest {
        verifier_id: state.server_config.frost_signer.identifier,
        out_point: request.outpoint,
        sats_fee_amount: request.sats_amount,
        status: deposit_status.clone(),
    };

    state
        .storage
        .set_status_and_fee_amount_by_out_point(request.outpoint, deposit_status, request.sats_amount)
        .await
        .map_err(|e| {
            VerifierError::Storage(format!(
                "Failed to update confirmation status and set sats fee amount: {}",
                e
            ))
        })?;

    state
        .gateway_client
        .notify_runes_deposit(gateway_request)
        .await
        .map_err(|e| VerifierError::GatewayClient(format!("Failed to notify runes deposit: {}", e)))?;

    tracing::info!("Runes deposit notified for out point: {}", request.outpoint);
    Ok(())
}
