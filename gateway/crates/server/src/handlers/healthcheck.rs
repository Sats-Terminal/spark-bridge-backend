use axum::{Json, extract::State};
use global_utils::common_resp::Empty;
use tracing::instrument;

use crate::{error::GatewayError, init::AppState};

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(State(state): State<AppState>) -> Result<Json<Empty>, GatewayError> {
    tracing::info!("Handling healthcheck request...");
    for verifier_client in state.verifier_clients.iter() {
        verifier_client.healthcheck().await.map_err(|_| {
            GatewayError::HealthcheckError(format!(
                "Failed to perform healthcheck for verifier client {}",
                verifier_client.get_id()
            ))
        })?;
    }
    Ok(Json(Empty {}))
}
