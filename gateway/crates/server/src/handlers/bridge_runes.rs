use crate::error::GatewayError;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct BridgeRunesToSparkRequest {
    pub user_id: String,
}

#[derive(Serialize)]
pub struct BridgeRunesToSparkResponse {
    pub message: String,
}

pub async fn handle(
    Json(request): Json<BridgeRunesToSparkRequest>,
) -> Result<Json<BridgeRunesToSparkResponse>, GatewayError> {
    Ok(Json(BridgeRunesToSparkResponse {
        message: "success".to_string(),
    }))
}
