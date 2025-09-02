use crate::errors::VerifierError;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct WatchRunesAddressRequest {
    pub address: String,
}

#[derive(Serialize)]
pub struct WatchRunesAddressResponse {
    pub partial_address: String,
}

pub async fn handle(
    Json(request): Json<WatchRunesAddressRequest>,
) -> Result<Json<WatchRunesAddressResponse>, VerifierError> {
    Ok(Json(WatchRunesAddressResponse {
        partial_address: "0x1234567890".to_string(),
    }))
}
