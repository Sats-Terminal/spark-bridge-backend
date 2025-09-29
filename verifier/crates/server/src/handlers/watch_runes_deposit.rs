use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::OutPoint;
use frost::types::Nonce;
use serde::{Deserialize, Serialize};
use verifier_btc_indexer_client::client::WatchRunesDepositRequest as IndexerWatchRunesDepositRequest;
use verifier_config_parser::config::construct_hardcoded_callback_url;
use verifier_local_db_store::schemas::deposit_address::{DepositAddrInfo, DepositAddressStorage, DepositStatus};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositRequest {
    pub musig_id: MusigId,
    pub nonce: Nonce,
    pub amount: u64,
    pub btc_address: String,
    pub bridge_address: String,
    pub out_point: OutPoint,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositResponse {}

pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<WatchRunesDepositRequest>,
) -> Result<Json<WatchRunesDepositResponse>, VerifierError> {
    state
        .storage
        .set_deposit_addr_info(DepositAddrInfo {
            user_uuid: todo!(),
            nonce: request.nonce,
            out_point: Some(request.out_point),
            deposit_address: request.btc_address.clone(),
            bridge_address: request.bridge_address.clone(),
            is_btc: false,
            deposit_amount: request.amount,
            sats_fee_amount: None,
            confirmation_status: DepositStatus::WaitingForConfirmation,
        })
        .await
        .map_err(|e| VerifierError::StorageError(format!("Failed to set deposit address info: {}", e)))?;

    let callback_url = construct_hardcoded_callback_url(&state.server_config.server);

    state
        .btc_indexer_client
        .watch_runes_deposit(IndexerWatchRunesDepositRequest {
            btc_address: request.btc_address,
            out_point: request.out_point,
            rune_id: request.musig_id.get_rune_id(),
            rune_amount: request.amount,
            callback_url,
        })
        .await
        .map_err(|e| VerifierError::BtcIndexerClientError(format!("Failed to watch runes deposit: {}", e)))?;

    Ok(Json(WatchRunesDepositResponse {}))
}
