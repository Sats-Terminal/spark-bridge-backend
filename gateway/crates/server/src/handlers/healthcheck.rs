use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use global_utils::common_resp::Empty;
use serde::{Deserialize, Serialize};
use tracing::{instrument, trace};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TestSparkRequest {
    pub btc_address: String,
}

#[instrument(level = "info", skip(state), ret)]
pub async fn handle(State(state): State<AppState>) -> Result<Json<Empty>, GatewayError> {
    trace!("Handling healthcheck request...",);
    state
        .deposit_verification_aggregator
        .healthcheck()
        .await
        .map_err(|e| GatewayError::HealthcheckError(e.to_string()))?;
    Ok(Json(Empty {}))
}
