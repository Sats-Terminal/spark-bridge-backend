use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use frost::types::{SignRound1Request, SignRound1Response};
use tracing::instrument;

#[instrument(level = "trace", skip_all)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<SignRound1Request>,
) -> Result<Json<SignRound1Response>, VerifierError> {
    let request_musig_id = request.musig_id.clone();
    tracing::info!("Sign round 1 for musig id: {:?}", request_musig_id);
    let response = state.frost_signer.sign_round_1(request.clone()).await?;
    tracing::info!("Sign round 1 success for musig id: {:?}", request_musig_id);
    Ok(Json(response))
}
