use crate::errors::VerifierError;
use axum::Json;
use crate::state::AppState;
use axum::extract::State;
use frost::traits::{DkgRound2Request, DkgRound2Response};


pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<DkgRound2Request>,
) -> Result<Json<DkgRound2Response>, VerifierError> {
    let response = state.frost_signer.dkg_round_2(request).await?;
    Ok(Json(response))
}
