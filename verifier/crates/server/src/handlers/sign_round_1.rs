use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use frost::types::{SignRound1Request, SignRound1Response};
use tracing::instrument;

#[instrument(level = "debug", skip_all, ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<SignRound1Request>,
) -> Result<Json<SignRound1Response>, VerifierError> {
    let response = state.frost_signer.sign_round_1(request).await?;
    tracing::debug!("[verifier] Sign round1 response: {:?}", response);
    Ok(Json(response))
}
