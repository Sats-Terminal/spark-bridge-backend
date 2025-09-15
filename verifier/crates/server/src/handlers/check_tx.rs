use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use btc_resp_aggregator::traits::{CheckTxRequest, CheckTxResponse};
use global_utils::api_result_request::ApiResponseOwned;
use tracing::instrument;

#[instrument(level = "debug", skip_all, ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<CheckTxRequest>,
) -> Result<Json<CheckTxResponse>, VerifierError> {
    let _ = state.btc_tx_checker.save_tx(&request).await?;
    let response = state
        .btc_tx_checker
        .subscribe_indexer_to_loopback_addr(request.tx_id)
        .await?;
    tracing::debug!("[verifier] DKG round2 response: {:?}", response);
    Ok(Json(CheckTxResponse {
        response: ApiResponseOwned::Ok { data: () },
    }))
}
