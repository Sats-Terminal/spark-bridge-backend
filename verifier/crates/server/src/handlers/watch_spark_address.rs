use crate::errors::VerifierError;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct WatchSparkAddressRequest {
    pub address: String,
}

#[derive(Serialize)]
pub struct WatchSparkAddressResponse {
    pub partial_address: String,
}

pub async fn handle(
    Json(request): Json<WatchSparkAddressRequest>,
) -> Result<Json<WatchSparkAddressResponse>, VerifierError> {
    Ok(Json(WatchSparkAddressResponse {
        partial_address: "0x1234567890".to_string(),
    }))
}
