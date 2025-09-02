use crate::errors::VerifierError;
use axum::Json;
use crate::state::AppState;
use axum::extract::State;

use frost::traits::{DkgRound1Request, DkgRound1Response};

pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<DkgRound1Request>,
) -> Result<Json<DkgRound1Response>, VerifierError> {
    let response = state.frost_signer.dkg_round_1(request).await?;
    Ok(Json(response))
}
