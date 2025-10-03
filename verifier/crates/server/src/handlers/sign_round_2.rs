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
    let dkg_share_id = request.dkg_share_id;
    tracing::info!(dkg_share_id = ?dkg_share_id, "Sign round 2 running..");
    let response = state.frost_signer.sign_round_2(request).await?;
    tracing::info!(dkg_share_id = ?dkg_share_id, "Sign round 2 success finalized");
    Ok(Json(response))
}
