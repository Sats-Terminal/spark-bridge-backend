use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use tracing::instrument;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct BridgeRunesSparkRequest {
    pub user_public_key: String,
    pub rune_id: String,
    pub amount: u64,
}

#[derive(Serialize, Debug)]
pub struct BridgeRunesSparkResponse {
    pub message: String,
}

#[instrument(level = "info", skip(request, state), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<BridgeRunesSparkRequest>,
) -> Result<Json<BridgeRunesSparkResponse>, GatewayError> {
    // todo add logic to minting tokens
    // todo: extract saved spark address
    Ok(Json(BridgeRunesSparkResponse {
        message: "success".to_string(),
    }))
}
