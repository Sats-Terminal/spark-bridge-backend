use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::OutPoint;
use frost::types::TweakBytes;
use serde::{Deserialize, Serialize};
use spark_balance_checker_server::models::VerifyTransferRequest;
use tracing::instrument;
use uuid::Uuid;
use verifier_btc_indexer_client::client::WatchRunesDepositRequest as IndexerWatchRunesDepositRequest;
use verifier_local_db_store::schemas::deposit_address::{
    DepositAddrInfo, DepositAddressStorage, DepositStatus, FeePayment, InnerAddress,
};
use verifier_local_db_store::schemas::user_identifier::{UserIdentifierStorage, UserIds};
use verifier_spark_balance_checker_client::client::cast_deposit_status;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositRequest {
    pub request_id: Uuid,
    pub user_ids: UserIds,
    pub nonce: TweakBytes,
    pub amount: u64,
    pub btc_address: String,
    pub bridge_address: String,
    pub outpoint: OutPoint,
    pub fee_payment: Option<FeePayment>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchRunesDepositResponse {}

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<WatchRunesDepositRequest>,
) -> Result<Json<WatchRunesDepositResponse>, VerifierError> {
    tracing::info!("Watching runes deposit for address: {}", request.btc_address);

    if state.server_config.fee.is_some() && request.fee_payment.is_none() {
        return Err(VerifierError::Validation("Fee payment is required".to_string()));
    }

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
            token: request.user_ids.rune_id.clone(),
            fee_payment: request.fee_payment.clone(),
            fee_payment_status: match state.server_config.fee {
                Some(_) => DepositStatus::Pending,
                None => DepositStatus::Confirmed,
            },
            deposit_address,
            bridge_address,
            is_btc: true,
            deposit_amount: request.amount,
            sats_amount: None,
            confirmation_status: DepositStatus::Pending,
            error_details: None,
        })
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set deposit address info: {}", e)))?;

    match state.server_config.fee {
        None => {
            let callback_url = state.server_config.server.callback_url.clone();
            state
                .btc_indexer_client
                .watch_runes_deposit(IndexerWatchRunesDepositRequest {
                    request_id: request.request_id,
                    btc_address: request.btc_address.clone(),
                    outpoint: request.outpoint,
                    rune_id: Some(request.user_ids.rune_id),
                    rune_amount: Some(request.amount),
                    sats_amount: None,
                    callback_url: callback_url.to_string(),
                })
                .await
                .map_err(|e| VerifierError::BtcIndexerClient(format!("Failed to watch runes deposit: {}", e)))?;
            return Ok(Json(WatchRunesDepositResponse {}));
        }
        Some(fee_cfg) => {
            // Safe to unwrap since none case was handled before
            match request.fee_payment.unwrap() {
                // In case of BTC fee payment, go on with async flow to handle the fee payment first
                // and then watch the rune deposit in the same way via btc_indexer callback notification
                FeePayment::Btc(outpoint) => {
                    state
                        .btc_indexer_client
                        .watch_runes_deposit(IndexerWatchRunesDepositRequest {
                            request_id: request.request_id,
                            btc_address: fee_cfg.btc_address.clone(),
                            outpoint,
                            rune_id: None,
                            rune_amount: None,
                            sats_amount: Some(fee_cfg.amount),
                            callback_url: state.server_config.server.callback_url.to_string(),
                        })
                        .await
                        .map_err(|e| {
                            VerifierError::BtcIndexerClient(format!("Failed to watch btc fee payment: {}", e))
                        })?;
                }
                // When the fee is paid with Spark satoshis, verify transaction synchronously and
                // only then send a call to the btc_indexer to verify runes deposit
                FeePayment::Spark(txid) => {
                    let response = state
                        .spark_balance_checker_client
                        .verify_transfer(VerifyTransferRequest {
                            request_id: request.request_id,
                            receiver_address: fee_cfg.spark_address.clone(),
                            txid: txid.clone(),
                            amount: fee_cfg.amount,
                        })
                        .await
                        .map_err(|e| {
                            VerifierError::SparkBalanceCheckerClient(format!("Failed to verify balance: {}", e))
                        })?;

                    state
                        .storage
                        .set_fee_status_by_payment(
                            FeePayment::Spark(txid.clone()),
                            cast_deposit_status(&response.deposit_status),
                            response.error_details.clone(),
                        )
                        .await
                        .map_err(|e| VerifierError::Storage(format!("Failed to set fee status: {}", e)))?;

                    let callback_url = state.server_config.server.callback_url.clone();

                    let deposit_addr_info = state
                        .storage
                        .get_deposit_addr_info_by_confirmed_fee_payment(FeePayment::Spark(txid.clone()))
                        .await
                        .map_err(|err| VerifierError::Storage(err.to_string()))?;

                    state
                        .btc_indexer_client
                        .watch_runes_deposit(IndexerWatchRunesDepositRequest {
                            request_id: request.request_id,
                            btc_address: deposit_addr_info.deposit_address.to_string(),
                            outpoint: deposit_addr_info.outpoint.unwrap_or_default(),
                            rune_id: Some(request.user_ids.rune_id),
                            rune_amount: Some(deposit_addr_info.deposit_amount),
                            sats_amount: None,
                            callback_url: callback_url.to_string(),
                        })
                        .await
                        .map_err(|e| {
                            VerifierError::BtcIndexerClient(format!("Failed to watch runes deposit: {}", e))
                        })?;
                }
            };
        }
    };

    tracing::info!("Runes deposit watched for address: {}", request.btc_address);

    Ok(Json(WatchRunesDepositResponse {}))
}
