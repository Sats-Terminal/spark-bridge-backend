use crate::error::GatewayError;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct NotifySparkAddressRequest {
    pub user_id: String,
}

#[derive(Serialize)]
pub struct NotifySparkAddressResponse {
    pub message: String,
}

pub async fn handle(
    Json(request): Json<NotifySparkAddressRequest>,
) -> Result<Json<NotifySparkAddressResponse>, GatewayError> {
    Ok(Json(NotifySparkAddressResponse {
        message: "success".to_string(),
    }))
}
