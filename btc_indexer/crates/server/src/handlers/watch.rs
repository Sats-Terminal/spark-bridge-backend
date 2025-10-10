use axum::Json;
use axum::extract::State;
use crate::init::AppState;
use crate::error::BtcIndexerServerError;
use bitcoin::OutPoint;
use serde::{Deserialize, Serialize};
use btc_indexer_local_db_store::WatchRequest as LocalDbWatchRequest;
use btc_indexer_local_db_store::WatchRequestStatus;
use ordinals::RuneId;
use std::str::FromStr;
use chrono::Utc;
use bitcoin::{Address, Network};
use url::Url;
use tracing::instrument;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct WatchRequest {
    pub outpoint: OutPoint,
    pub btc_address: String,
    pub rune_id: Option<String>,
    pub rune_amount: Option<u64>,
    pub sats_amount: Option<u64>,
    pub callback_url: String,
}

#[instrument(level = "trace", skip(state))]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<WatchRequest>,
) -> Result<Json<()>, BtcIndexerServerError> {
    let btc_address = request.btc_address.clone();
    tracing::info!("Watching request: {:?}", btc_address);
    let rune_id = match request.rune_id {
        Some(rune_id) => Some(validate_rune_id(rune_id)?),
        None => None,
    };
    let callback_url = Url::parse(&request.callback_url).map_err(|e| BtcIndexerServerError::ValidationError(format!("Invalid callback url: {}", e)))?;
    let local_db_watch_request = LocalDbWatchRequest {
        outpoint: request.outpoint,
        btc_address: validate_address(request.btc_address, state.network)?,
        rune_id,
        rune_amount: request.rune_amount.map(|rune_amount| rune_amount as u128),
        sats_amount: request.sats_amount,
        created_at: get_current_timestamp(),
        status: WatchRequestStatus::Pending,
        error_details: None,
        callback_url,
    };
    state.storage.insert_watch_request(local_db_watch_request).await?;
    tracing::info!("Watching request inserted for btc address: {:?}", btc_address);
    Ok(Json(()))
}

pub fn validate_rune_id(rune_id: String) -> Result<RuneId, BtcIndexerServerError> {
    let rune_id = RuneId::from_str(&rune_id).map_err(|e| BtcIndexerServerError::ValidationError(format!("Invalid rune id: {}", e)))?;
    Ok(rune_id)
}

pub fn get_current_timestamp() -> u64 {
    Utc::now().timestamp_millis() as u64
}

pub fn validate_address(address: String, network: Network) -> Result<Address, BtcIndexerServerError> {
    let address = Address::from_str(&address)
        .map_err(|e| BtcIndexerServerError::ValidationError(format!("Invalid address: {}", e)))?
        .require_network(network)
        .map_err(|e| BtcIndexerServerError::ValidationError(format!("Invalid network: {}", e)))?;
    Ok(address)
}
