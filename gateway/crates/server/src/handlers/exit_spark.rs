use crate::error::GatewayError;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ExitSparkRequest {
    pub user_id: String,
}

#[derive(Serialize)]
pub struct ExitSparkResponse {
    pub message: String,
}

pub async fn handle(Json(request): Json<ExitSparkRequest>) -> Result<Json<ExitSparkResponse>, GatewayError> {
    Ok(Json(ExitSparkResponse {
        message: "success".to_string(),
    }))
}
