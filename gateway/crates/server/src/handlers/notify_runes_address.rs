use crate::error::GatewayError;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct NotifyRunesAddressRequest {
    pub user_id: String,
}

#[derive(Serialize)]
pub struct NotifyRunesAddressResponse {
    pub message: String,
}

pub async fn handle(
    Json(request): Json<NotifyRunesAddressRequest>,
) -> Result<Json<NotifyRunesAddressResponse>, GatewayError> {
    Ok(Json(NotifyRunesAddressResponse {
        message: "success".to_string(),
    }))
}
