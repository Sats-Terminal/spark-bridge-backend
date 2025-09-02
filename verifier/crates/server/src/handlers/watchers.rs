use axum::Json;

use crate::{
    errors::VerifierError,
    handlers::types::{
        WatchRunesAddressRequest, WatchRunesAddressResponse, WatchSparkAddressRequest, WatchSparkAddressResponse,
    },
};

pub async fn handler_watch_spark_address(
    Json(request): Json<WatchSparkAddressRequest>,
) -> Result<Json<WatchSparkAddressResponse>, VerifierError> {
    Ok(Json(WatchSparkAddressResponse {
        partial_address: "0x1234567890".to_string(),
    }))
}

pub async fn handler_watch_runes_address(
    Json(request): Json<WatchRunesAddressRequest>,
) -> Result<Json<WatchRunesAddressResponse>, VerifierError> {
    Ok(Json(WatchRunesAddressResponse {
        partial_address: "0x1234567890".to_string(),
    }))
}
