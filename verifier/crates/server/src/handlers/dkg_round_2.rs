use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use frost::types::{DkgRound2Request, DkgRound2Response};
use tracing::instrument;

#[instrument(level = "trace", skip_all)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<DkgRound2Request>,
) -> Result<Json<DkgRound2Response>, VerifierError> {
    let request_musig_id = request.musig_id.clone();
    tracing::info!("DKG round 2 for musig id: {:?}", request_musig_id);
    let response = state.frost_signer.dkg_round_2(request.clone()).await?;
    tracing::info!("DKG round 2 success for musig id: {:?}", request_musig_id);
    Ok(Json(response))
}
