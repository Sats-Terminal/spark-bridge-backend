use crate::error::GatewayError;
use crate::state::AppState;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct GetRunesAddressRequest {
    pub user_public_key: String,
    pub rune_id: String,
    //todo: add amount
}

#[derive(Serialize)]
pub struct GetRunesAddressResponse {
    pub address: String,
}

pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<GetRunesAddressRequest>,
) -> Result<Json<GetRunesAddressResponse>, GatewayError> {
    // todo:
    Ok(Json(GetRunesAddressResponse {
        address: request.user_public_key,
    }))
}
