use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use frost::types::{SignRound2Request, SignRound2Response};
use tracing::instrument;

#[instrument(level = "debug", skip_all, ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<SignRound2Request>,
) -> Result<Json<SignRound2Response>, VerifierError> {
    let response = state.frost_signer.sign_round_2(request).await?;
    tracing::debug!("[verifier] Sign round2 response: {:?}", response);
    Ok(Json(response))
}
