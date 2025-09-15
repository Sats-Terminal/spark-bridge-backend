use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use btc_resp_aggregator::aggregator::BtcConfirmationsAggregator;
use btc_resp_aggregator::traits::CheckTxRequest;
use gateway_api::api::{BridgeRunesToSparkRequest, BridgeRunesToSparkResponse};
use global_utils::common_types::{TxIdWrapped, get_uuid};
use global_utils::network::convert_to_http_url;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::instrument;
use uuid::Uuid;

#[instrument(level = "info", skip(request, state), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<BridgeRunesToSparkRequest>,
) -> Result<Json<BridgeRunesToSparkResponse>, GatewayError> {
    // todo add logic to minting tokens
    // todo: extract saved spark address
    Ok(Json(BridgeRunesToSparkResponse {
        message: "success".to_string(),
    }))
}
