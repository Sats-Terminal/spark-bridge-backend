use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use gateway_deposit_verification::types::NotifyRunesDepositRequest;
use tracing::instrument;

#[instrument(level = "info", skip(request, state), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<NotifyRunesDepositRequest>,
) -> Result<Json<()>, GatewayError> {
    // TODO: This request should spawn task and immediately return Json(())
    let _ = state
        .deposit_verification_aggregator
        .notify_runes_deposit(request)
        .await;

    Ok(Json(()))
}
