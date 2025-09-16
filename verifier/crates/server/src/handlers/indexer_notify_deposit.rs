use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use btc_indexer_api::api::BtcIndexerCallbackResponse;
use global_utils::api_result_request::Empty;
use tracing::instrument;

pub type LoopbackBtcIndexerResponse = Empty;

#[instrument(level = "debug", skip_all, ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<BtcIndexerCallbackResponse>,
) -> Result<Json<Empty>, VerifierError> {
    Ok(Json(Empty {}))
}
