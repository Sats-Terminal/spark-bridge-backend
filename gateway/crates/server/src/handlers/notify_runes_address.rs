use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use global_utils::common_types::{TxIdWrapped, get_uuid};
use global_utils::network::convert_to_http_url;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::instrument;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct NotifyRunesAddressRequest {
    pub address: String,
}

#[derive(Serialize, Debug)]
pub struct NotifyRunesAddressResponse {
    pub address: String,
}

#[instrument(level = "info", skip(request, state), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<NotifyRunesAddressRequest>,
) -> Result<Json<NotifyRunesAddressResponse>, GatewayError> {
    Ok(Json(NotifyRunesAddressResponse {
        address: request.address,
    }))
}
