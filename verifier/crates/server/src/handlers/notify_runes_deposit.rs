use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use btc_indexer_api::api::BtcIndexerCallbackResponse;
use tracing::instrument;
use serde::{Deserialize, Serialize};
use verifier_local_db_store::schemas::deposit_address::DepositStatus;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NotifySparkDepositResponse {
    pub verifier_response: DepositStatus,
}

#[instrument(level = "debug", skip_all, ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<BtcIndexerCallbackResponse>,
) -> Result<Json<NotifySparkDepositResponse>, VerifierError> {
    
    // TODO: when we will have proper indexer logic, we will update the confirmation status here

    Ok(Json(NotifySparkDepositResponse {
        verifier_response: DepositStatus::Confirmed,
    }))
}
