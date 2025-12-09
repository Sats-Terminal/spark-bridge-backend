use axum::{Json, extract::State};
use frost::types::{DkgRound1Request, DkgRound1Response};
use tracing::instrument;

use crate::{errors::VerifierError, init::AppState};

#[instrument(level = "trace", skip_all)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<DkgRound1Request>,
) -> Result<Json<DkgRound1Response>, VerifierError> {
    let dkg_share_id = request.dkg_share_id;
    tracing::info!(dkg_share_id = ?dkg_share_id, "DKG round 1 running..");
    let response = state.frost_signer.dkg_round_1(request).await?;
    tracing::info!(dkg_share_id = ?dkg_share_id, "DKG round 1 success");
    Ok(Json(response))
}
