use crate::error::GatewayError;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct GetRunesAddressRequest {
    pub user_id: String,
    pub rune_id: String,
}

#[derive(Serialize)]
pub struct GetRunesAddressResponse {
    pub address: String,
}

pub async fn handle(
    Json(request): Json<GetRunesAddressRequest>,
) -> Result<Json<GetRunesAddressResponse>, GatewayError> {
    Ok(Json(GetRunesAddressResponse {
        address: format!("user_id: {}", request.user_id),
    }))
}
