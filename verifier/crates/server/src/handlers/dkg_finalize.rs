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
    let request_musig_id = request.musig_id.clone();
    tracing::info!("DKG finalize for musig id: {:?}", request_musig_id);
    let response = state.frost_signer.dkg_finalize(request).await?;
    tracing::info!("DKG finalize success for musig id: {:?}", request_musig_id);
    Ok(Json(response))
}
