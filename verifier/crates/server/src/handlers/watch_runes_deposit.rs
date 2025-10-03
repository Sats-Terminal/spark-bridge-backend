use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::OutPoint;
use frost::types::MusigId;
use frost::types::TweakBytes;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use verifier_btc_indexer_client::client::WatchRunesDepositRequest as IndexerWatchRunesDepositRequest;
use verifier_config_parser::config::construct_hardcoded_callback_url;
use verifier_local_db_store::schemas::deposit_address::{
    DepositAddrInfo, DepositAddressStorage, DepositStatus, InnerAddress,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositRequest {
    pub musig_id: MusigId,
    pub nonce: TweakBytes,
    pub amount: u64,
    pub btc_address: String,
    pub bridge_address: String,
    pub out_point: OutPoint,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositResponse {}

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<WatchRunesDepositRequest>,
) -> Result<Json<WatchRunesDepositResponse>, VerifierError> {
    tracing::info!("Watching runes deposit for address: {}", request.btc_address);

    let deposit_address = InnerAddress::from_string_and_type(request.btc_address.clone(), true)
        .map_err(|e| VerifierError::Validation(format!("Invalid BTC address: {}", e)))?;
    let bridge_address = InnerAddress::SparkAddress(request.bridge_address.clone());

    state
        .storage
        .set_deposit_addr_info(DepositAddrInfo {
            musig_id: request.musig_id.clone(),
            nonce: request.nonce,
            out_point: Some(request.out_point),
            deposit_address,
            bridge_address,
            is_btc: true, // ??
            deposit_amount: request.amount,
            sats_fee_amount: None,
            confirmation_status: DepositStatus::WaitingForConfirmation,
        })
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set deposit address info: {}", e)))?;

    let callback_url = construct_hardcoded_callback_url(&state.server_config.server);

    state
        .btc_indexer_client
        .watch_runes_deposit(IndexerWatchRunesDepositRequest {
            btc_address: request.btc_address.clone(),
            out_point: request.out_point,
            rune_id: request.musig_id.get_rune_id(),
            rune_amount: request.amount,
            callback_url,
        })
        .await
        .map_err(|e| VerifierError::BtcIndexerClient(format!("Failed to watch runes deposit: {}", e)))?;

    tracing::info!("Runes deposit watched for address: {}", request.btc_address);

    Ok(Json(WatchRunesDepositResponse {}))
}
