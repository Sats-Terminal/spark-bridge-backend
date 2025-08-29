use axum::{
    Json,
};
use serde::{Deserialize, Serialize};
use crate::errors::VerifierError;

#[derive(Deserialize)]
pub struct WatchSparkAddressRequest {
    pub address: String,
}

#[derive(Serialize)]
pub struct WatchSparkAddressResponse {
    pub partial_address: String,
}

pub async fn handler_watch_spark_address(Json(request): Json<WatchSparkAddressRequest>) -> Result<Json<WatchSparkAddressResponse>, VerifierError> {
    Ok(Json(WatchSparkAddressResponse {
        partial_address: "0x1234567890".to_string(),
    }))
}

#[derive(Deserialize)]
pub struct WatchRunesAddressRequest {
    pub address: String,
}

#[derive(Serialize)]
pub struct WatchRunesAddressResponse {
    pub partial_address: String,
}

pub async fn handler_watch_runes_address(Json(request): Json<WatchRunesAddressRequest>) -> Result<Json<WatchRunesAddressResponse>, VerifierError> {
    Ok(Json(WatchRunesAddressResponse {
        partial_address: "0x1234567890".to_string(),
    }))
}

#[derive(Deserialize)]
pub struct GetPublicKeyPackageRequest {
    pub key_id: String,
}

#[derive(Serialize)]
pub struct GetPublicKeyPackageResponse {
    pub public_key_package: String,
}

pub async fn handler_get_public_key_package(Json(request): Json<GetPublicKeyPackageRequest>) -> Result<Json<GetPublicKeyPackageResponse>, VerifierError> {
    Ok(Json(GetPublicKeyPackageResponse {
        public_key_package: "0x1234567890".to_string(),
    }))
}