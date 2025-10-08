use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use frost::types::{DkgFinalizeRequest, DkgFinalizeResponse};
use tracing::instrument;

#[instrument(level = "trace", skip_all)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<DkgFinalizeRequest>,
) -> Result<Json<DkgFinalizeResponse>, VerifierError> {
    let dkg_share_id = request.dkg_share_id;
    tracing::info!(dkg_share_id = ?dkg_share_id, "DKG finalize running..");
    let response = state.frost_signer.dkg_finalize(request).await?;
    tracing::info!(dkg_share_id = ?dkg_share_id, "DKG finalize finalized");
    Ok(Json(response))
}
