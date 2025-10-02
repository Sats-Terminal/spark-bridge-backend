use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use frost::types::{SignRound2Request, SignRound2Response};
use tracing::instrument;

#[instrument(level = "trace", skip_all)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<SignRound2Request>,
) -> Result<Json<SignRound2Response>, VerifierError> {
    let request_musig_id = request.musig_id.clone();
    tracing::info!("Sign round 2 for musig id: {:?}", request_musig_id);
    let response = state.frost_signer.sign_round_2(request.clone()).await?;
    tracing::info!("Sign round 2 success for musig id: {:?}", request_musig_id);
    Ok(Json(response))
}
