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
    let dkg_share_id = request.dkg_share_id;
    tracing::info!(dkg_share_id = ?dkg_share_id, "Sign round 1 running..");
    let response = state.frost_signer.sign_round_1(request).await?;
    tracing::info!(dkg_share_id = ?dkg_share_id, "Sign round 1 success finalized");
    Ok(Json(response))
}
