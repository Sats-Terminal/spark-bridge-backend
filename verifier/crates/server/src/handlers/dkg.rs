use axum::Json;

use crate::{
    errors::VerifierError,
    handlers::types::{
        GetRound1PackageRequest, GetRound1PackageResponse, GetRound2PackageRequest, GetRound2PackageResponse,
        GetRound3PackageRequest, GetRound3PackageResponse,
    },
};

pub async fn handler_get_round_1_package(
    Json(request): Json<GetRound1PackageRequest>,
) -> Result<Json<GetRound1PackageResponse>, VerifierError> {
    Ok(Json(GetRound1PackageResponse {
        round_1_package: "0x1234567890".to_string(),
    }))
}

pub async fn handler_get_round_2_package(
    Json(request): Json<GetRound2PackageRequest>,
) -> Result<Json<GetRound2PackageResponse>, VerifierError> {
    Ok(Json(GetRound2PackageResponse {
        round_2_package: "0x1234567890".to_string(),
    }))
}

pub async fn handler_get_round_3_package(
    Json(request): Json<GetRound3PackageRequest>,
) -> Result<Json<GetRound3PackageResponse>, VerifierError> {
    Ok(Json(GetRound3PackageResponse {
        final_key_package: "0x1234567890".to_string(),
    }))
}
