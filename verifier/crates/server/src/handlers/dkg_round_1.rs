use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use frost::types::{DkgRound1Request, DkgRound1Response};
use tracing::instrument;

#[instrument(level = "trace", skip_all)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<DkgRound1Request>,
) -> Result<Json<DkgRound1Response>, VerifierError> {
    let request_musig_id = request.musig_id.clone();
    tracing::info!("DKG round 1 for musig id: {:?}", request_musig_id);
    let response = state.frost_signer.dkg_round_1(request).await?;
    tracing::info!("DKG round 1 success for musig id: {:?}", request_musig_id);
    Ok(Json(response))
}
