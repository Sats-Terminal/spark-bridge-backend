use crate::errors::VerifierError;
use axum::Json;
use crate::handlers::types::{
    GetPublicKeyPackageRequest,
    GetPublicKeyPackageResponse,
};

pub async fn handler_get_public_key_package(Json(request): Json<GetPublicKeyPackageRequest>) -> Result<Json<GetPublicKeyPackageResponse>, VerifierError> {
    Ok(Json(GetPublicKeyPackageResponse {
        public_key_package: "0x1234567890".to_string(),
    }))
}