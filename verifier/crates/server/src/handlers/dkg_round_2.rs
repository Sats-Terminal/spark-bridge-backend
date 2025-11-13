use axum::{Json, extract::State};
use frost::types::{DkgRound2Request, DkgRound2Response};
use tracing::instrument;

use crate::{errors::VerifierError, init::AppState};

#[instrument(level = "trace", skip_all)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<DkgRound2Request>,
) -> Result<Json<DkgRound2Response>, VerifierError> {
    let dkg_share_id = request.dkg_share_id;
    tracing::info!(dkg_share_id = ?dkg_share_id, "DKG round 2 running..");
    let response = state.frost_signer.dkg_round_2(request).await?;
    tracing::info!(dkg_share_id = ?dkg_share_id, "DKG round 2 finalized");
    Ok(Json(response))
}
