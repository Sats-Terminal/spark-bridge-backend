use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::OutPoint;
use frost::types::TweakBytes;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use verifier_btc_indexer_client::client::WatchRunesDepositRequest as IndexerWatchRunesDepositRequest;
use verifier_config_parser::config::construct_hardcoded_callback_url;
use verifier_local_db_store::schemas::deposit_address::{
    DepositAddrInfo, DepositAddressStorage, DepositStatus, InnerAddress,
};
use verifier_local_db_store::schemas::user_identifier::{UserIdentifierStorage, UserIds};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositRequest {
    pub request_id: Uuid,
    pub user_ids: UserIds,
    pub nonce: TweakBytes,
    pub amount: u64,
    pub btc_address: String,
    pub bridge_address: String,
    pub outpoint: OutPoint,
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
        .insert_user_ids(request.user_ids.clone())
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set identifier data: {}", e)))?;

    state
        .storage
        .insert_deposit_addr_info(DepositAddrInfo {
            dkg_share_id: request.user_ids.dkg_share_id,
            nonce: request.nonce,
            outpoint: Some(request.outpoint),
            deposit_address,
            bridge_address,
            is_btc: false,
            deposit_amount: request.amount,
            sats_amount: None,
            confirmation_status: DepositStatus::Pending,
            error_details: None,
        })
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set deposit address info: {}", e)))?;

    let callback_url = construct_hardcoded_callback_url(&state.server_config.server);

    state
        .btc_indexer_client
        .watch_runes_deposit(IndexerWatchRunesDepositRequest {
            request_id: request.request_id,
            btc_address: request.btc_address.clone(),
            outpoint: request.outpoint,
            rune_id: Some(request.user_ids.rune_id.to_string()),
            rune_amount: Some(request.amount),
            sats_amount: None,
            callback_url: callback_url.to_string(),
        })
        .await
        .map_err(|e| VerifierError::BtcIndexerClient(format!("Failed to watch runes deposit: {}", e)))?;

    tracing::info!("Runes deposit watched for address: {}", request.btc_address);

    Ok(Json(WatchRunesDepositResponse {}))
}
