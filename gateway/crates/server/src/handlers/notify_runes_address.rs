use crate::error::GatewayError;
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Deserialize, Debug)]
pub struct NotifyRunesAddressRequest {
    pub user_id: String,
    pub spark_address: String,
}

#[derive(Serialize, Debug)]
pub struct NotifyRunesAddressResponse {
    pub message: String,
}

#[instrument(level = "info", skip(request), fields(request = ?request), ret)]
pub async fn handle(
    Json(request): Json<NotifyRunesAddressRequest>,
) -> Result<Json<NotifyRunesAddressResponse>, GatewayError> {
    //todo: accept request, send tx_id to track to validators and return ok
    Ok(Json(NotifyRunesAddressResponse {
        message: "success".to_string(),
    }))
}
