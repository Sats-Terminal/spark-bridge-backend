use crate::errors::VerifierError;
use axum::Json;
use crate::state::AppState;
use axum::extract::State;
use frost::traits::{DkgFinalizeRequest, DkgFinalizeResponse};
use tracing::instrument;

#[instrument(level = "debug", skip_all, ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<DkgFinalizeRequest>,
) -> Result<Json<DkgFinalizeResponse>, VerifierError> {
    let response = state.frost_signer.dkg_finalize(request).await?;
    tracing::debug!("DKG finalize response: {:?}", response);

    Ok(Json(response))
}
