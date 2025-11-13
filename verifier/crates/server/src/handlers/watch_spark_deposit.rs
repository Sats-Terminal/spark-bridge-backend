use axum::{Json, extract::State};
use frost::types::TweakBytes;
use serde::{Deserialize, Serialize};
use spark_balance_checker_server::models::{VerifyBalanceRequest, VerifyTransferRequest};
use token_identifier::TokenIdentifier;
use tracing::{self, instrument};
use uuid::Uuid;
use verifier_btc_indexer_client::client::WatchRunesDepositRequest as IndexerWatchRunesDepositRequest;
use verifier_local_db_store::schemas::{
    deposit_address::{DepositAddrInfo, DepositAddressStorage, DepositStatus, FeePayment, InnerAddress},
    user_identifier::{UserIdentifierStorage, UserIds},
};
use verifier_spark_balance_checker_client::client::cast_deposit_status;

use crate::{errors::VerifierError, init::AppState};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchSparkDepositRequest {
    pub request_id: Uuid,
    pub user_ids: UserIds,
    pub nonce: TweakBytes,
    pub spark_address: String,
    pub exit_address: String,
    pub amount: u64,
    pub token_identifier: TokenIdentifier,
    pub fee_payment: Option<FeePayment>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchSparkDepositResponse {
    pub verifier_response: DepositStatus,
}

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<WatchSparkDepositRequest>,
) -> Result<Json<WatchSparkDepositResponse>, VerifierError> {
    tracing::info!("Watching spark deposit for address: {}", request.spark_address);

    if state.server_config.fee.is_some() && request.fee_payment.is_none() {
        return Err(VerifierError::Validation("Fee payment is required".to_string()));
    }

    let deposit_address = InnerAddress::SparkAddress(request.spark_address.clone());
    let bridge_address = InnerAddress::from_string_and_type(request.exit_address.clone(), true)
        .map_err(|e| VerifierError::Validation(format!("Invalid exit address: {}", e)))?;

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
            outpoint: None,
            token: request.token_identifier.to_string(),
            fee_payment: request.fee_payment.clone(),
            fee_payment_status: match state.server_config.fee {
                Some(_) => DepositStatus::Pending,
                None => DepositStatus::Confirmed,
            },
            deposit_address: deposit_address.clone(),
            bridge_address,
            is_btc: false,
            deposit_amount: request.amount,
            sats_amount: None,
            confirmation_status: DepositStatus::Pending,
            error_details: None,
        })
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set deposit address info: {}", e)))?;

    let status = match state.server_config.fee.clone() {
        None => verify_spark_deposit(&state, &request, deposit_address).await?,
        Some(fee_cfg) => {
            // Safe to unwrap since none case was handled before
            match request.fee_payment.clone().unwrap() {
                // In case of BTC fee payment, go on with async flow to handle the fee payment first
                // and then Spark deposit synchronously
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

                    // Return 'pending' status to wait for the fee payment to be confirmed with
                    // callback notification and only then to handle Spark deposit itself
                    DepositStatus::Pending
                }
                // When the fee is paid using Spark satoshis, verify fee transaction and Spark
                // token deposit synchronously
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

                    verify_spark_deposit(&state, &request, deposit_address).await?
                }
            }
        }
    };

    tracing::info!("Spark deposit watched for address: {}", request.spark_address);

    Ok(Json(WatchSparkDepositResponse {
        verifier_response: status.clone(),
    }))
}

async fn verify_spark_deposit(
    state: &AppState,
    request: &WatchSparkDepositRequest,
    deposit_address: InnerAddress,
) -> Result<DepositStatus, VerifierError> {
    tracing::info!("Verifying balance for spark address: {}", request.spark_address);

    let response = state
        .spark_balance_checker_client
        .verify_balance(VerifyBalanceRequest {
            request_id: request.request_id,
            spark_address: request.spark_address.clone(),
            token_identifier: request.token_identifier,
            amount: request.amount as u128,
        })
        .await
        .map_err(|e| VerifierError::SparkBalanceCheckerClient(format!("Failed to verify balance: {}", e)))?;

    tracing::info!("response: {:?}", response);

    let status = cast_deposit_status(&response.deposit_status);
    state
        .storage
        .set_confirmation_status_by_deposit_address(deposit_address, status.clone(), response.error_details)
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to update confirmation status: {}", e)))?;

    Ok(status)
}
