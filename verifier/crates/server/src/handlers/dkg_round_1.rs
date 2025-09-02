use crate::errors::VerifierError;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct GetRound1PackageRequest {
    pub metadata: String,
}

#[derive(Serialize)]
pub struct GetRound1PackageResponse {
    pub round_1_package: String,
}

pub async fn handle(
    Json(request): Json<GetRound1PackageRequest>,
) -> Result<Json<GetRound1PackageResponse>, VerifierError> {
    Ok(Json(GetRound1PackageResponse {
        round_1_package: "0x1234567890".to_string(),
    }))
}
