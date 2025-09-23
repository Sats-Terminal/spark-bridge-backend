use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::Address;
use gateway_flow_processor::flow_sender::TypedMessageSender;
use gateway_flow_processor::types::BridgeRunesRequest;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::instrument;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TestSparkRequest {
    pub btc_address: String,
}

#[instrument(level = "info", skip(request, state), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<TestSparkRequest>,
) -> Result<Json<()>, GatewayError> {
    let address = Address::from_str(&request.btc_address)
        .unwrap()
        .require_network(state.network)
        .unwrap();

    state
        .flow_sender
        .send(BridgeRunesRequest { btc_address: address })
        .await
        .map_err(|e| GatewayError::FlowProcessorError(format!("Failed to test spark: {e}")))?;

    Ok(Json(()))
}
