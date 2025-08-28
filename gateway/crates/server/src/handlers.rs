use axum::{
    extract::Json,
    
};
use crate::errors::GatewayError;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct GetRunesAddressRequest {
    pub user_id: String,
}

#[derive(Serialize)]
pub struct GetRunesAddressResponse {
    pub address: String,
}

pub async fn handler_get_runes_address(Json(request): Json<GetRunesAddressRequest>) -> Result<Json<GetRunesAddressResponse>, GatewayError> {
    Ok(Json(GetRunesAddressResponse {
        address: format!("user_id: {}", request.user_id),
    }))
}

#[derive(Deserialize)]
pub struct BridgeRunesToSparkRequest {
    pub user_id: String,
}

#[derive(Serialize)]
pub struct BridgeRunesToSparkResponse {
    pub message: String,
}

pub async fn handler_bridge_runes(Json(request): Json<BridgeRunesToSparkRequest>) -> Result<Json<BridgeRunesToSparkResponse>, GatewayError> {
    Ok(Json(BridgeRunesToSparkResponse {
        message: "success".to_string(),
    }))
}

#[derive(Deserialize)]
pub struct ExitSparkRequest {
    pub user_id: String,
}

#[derive(Serialize)]
pub struct ExitSparkResponse {
    pub message: String,
}

pub async fn handler_exit_spark(Json(request): Json<ExitSparkRequest>) -> Result<Json<ExitSparkResponse>, GatewayError> {
    Ok(Json(ExitSparkResponse {
        message: "success".to_string(),
    }))
}