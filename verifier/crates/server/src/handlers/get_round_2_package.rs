use crate::errors::VerifierError;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct GetRound2PackageRequest {
    pub metadata: String,
}

#[derive(Serialize)]
pub struct GetRound2PackageResponse {
    pub round_2_package: String,
}

pub async fn handle(
    Json(request): Json<GetRound2PackageRequest>,
) -> Result<Json<GetRound2PackageResponse>, VerifierError> {
    Ok(Json(GetRound2PackageResponse {
        round_2_package: "0x1234567890".to_string(),
    }))
}
