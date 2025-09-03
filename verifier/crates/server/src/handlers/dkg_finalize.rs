use crate::errors::VerifierError;
use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use frost::traits::{DkgFinalizeRequest, DkgFinalizeResponse};

pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<DkgFinalizeRequest>,
) -> Result<Json<DkgFinalizeResponse>, VerifierError> {
    let response = state.frost_signer.dkg_finalize(request).await?;
    Ok(Json(response))
}
