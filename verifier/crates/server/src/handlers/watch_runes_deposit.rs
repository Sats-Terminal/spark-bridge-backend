use crate::errors::VerifierError;
use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use frost::types::MusigId;
use verifier_local_db_store::schemas::deposit_address::{DepositAddressStorage, DepositAddrInfo, DepositStatus};
use frost::types::Nonce;
use crate::init::AppState;
use bitcoin::Txid;
use verifier_btc_indexer_client::client::WatchRunesDepositRequest as IndexerWatchRunesDepositRequest;
use verifier_config_parser::config::construct_hardcoded_callback_url;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositRequest {
    pub musig_id: MusigId,
    pub nonce: Nonce,
    pub address: String,
    pub amount: u64,
    pub btc_address: String,
    pub txid: Txid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositResponse {}


pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<WatchRunesDepositRequest>,
) -> Result<Json<WatchRunesDepositResponse>, VerifierError> {
    state.storage.set_deposit_addr_info(
        &request.musig_id,
        request.nonce,
        DepositAddrInfo {
            address: request.address,
            is_btc: false,
            amount: request.amount,
            txid: None,
            confirmation_status: DepositStatus::WaitingForConfirmation,
        },
    ).await.map_err(|e| VerifierError::StorageError(format!("Failed to set deposit address info: {}", e)))?;

    let callback_url = construct_hardcoded_callback_url(&state.server_config.server);

    state.btc_indexer_client.watch_runes_deposit(IndexerWatchRunesDepositRequest {
        tx_id: request.txid,
        callback_url,
    }).await.map_err(|e| VerifierError::BtcIndexerClientError(format!("Failed to watch runes deposit: {}", e)))?;

    Ok(Json(WatchRunesDepositResponse {}))
}
