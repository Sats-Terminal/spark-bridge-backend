use crate::errors::VerifierError;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct GetRound3PackageRequest {
    pub metadata: String,
}

#[derive(Serialize)]
pub struct GetRound3PackageResponse {
    pub final_key_package: String,
}
pub async fn handle(
    Json(request): Json<GetRound3PackageRequest>,
) -> Result<Json<GetRound3PackageResponse>, VerifierError> {
    Ok(Json(GetRound3PackageResponse {
        final_key_package: "0x1234567890".to_string(),
    }))
}
