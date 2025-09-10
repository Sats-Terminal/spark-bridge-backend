use crate::error::GatewayError;
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Deserialize, Debug)]
pub struct BridgeRunesToSparkRequest {
    pub user_id: String,
}

#[derive(Serialize, Debug)]
pub struct BridgeRunesToSparkResponse {
    pub message: String,
}

#[instrument(level = "info", skip(request), fields(request = ?request), ret)]
pub async fn handle(
    Json(request): Json<BridgeRunesToSparkRequest>,
) -> Result<Json<BridgeRunesToSparkResponse>, GatewayError> {
    Ok(Json(BridgeRunesToSparkResponse {
        message: "success".to_string(),
    }))
}
