use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use frost::types::MusigId;
use frost::types::Nonce;
use serde::{Deserialize, Serialize};
use verifier_local_db_store::schemas::deposit_address::DepositAddressStorage;
use verifier_local_db_store::schemas::deposit_address::{DepositAddrInfo, DepositStatus, TxRejectReason, InnerAddress};
use verifier_spark_balance_checker_client::client::GetBalanceRequest;
use tracing;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchSparkDepositRequest {
    pub musig_id: MusigId,
    pub nonce: Nonce,
    pub spark_address: String,
    pub exit_address: String,
    pub amount: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchSparkDepositResponse {
    pub verifier_response: DepositStatus,
}

pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<WatchSparkDepositRequest>,
) -> Result<Json<WatchSparkDepositResponse>, VerifierError> {
    tracing::info!("Watching spark deposit for address: {}", request.spark_address);

    let deposit_address = InnerAddress::SparkAddress(request.spark_address.clone());
    let bridge_address = InnerAddress::from_string_and_type(request.exit_address, true)
        .map_err(|e| VerifierError::ValidationError(format!("Invalid exit address: {}", e)))?;

    state
        .storage
        .set_deposit_addr_info(DepositAddrInfo {
            musig_id: request.musig_id.clone(),
            nonce: request.nonce,
            out_point: None,
            deposit_address: deposit_address.clone(),
            bridge_address,
            is_btc: false, // ??
            deposit_amount: request.amount,
            sats_fee_amount: None,
            confirmation_status: DepositStatus::WaitingForConfirmation,
        })
        .await
        .map_err(|e| VerifierError::StorageError(format!("Failed to set deposit address info: {}", e)))?;

    let response = state
        .spark_balance_checker_client
        .get_balance(GetBalanceRequest {
            spark_address: request.spark_address.clone(),
            rune_id: request.musig_id.get_rune_id(),
        })
        .await
        .map_err(|e| VerifierError::SparkBalanceCheckerClientError(format!("Failed to get balance: {}", e)))?;

    let confirmation_status = match response.balance == request.amount as u128 {
        true => DepositStatus::Confirmed,
        false => DepositStatus::Failed(TxRejectReason::TooFewSatoshiPaidAsFee {
            got: response.balance as u64,
            at_least_expected: request.amount,
        }),
    };

    state
        .storage
        .set_confirmation_status_by_deposit_address(deposit_address, confirmation_status.clone())
        .await
        .map_err(|e| VerifierError::StorageError(format!("Failed to update confirmation status: {}", e)))?;

    tracing::debug!("Spark deposit watched for address: {}", request.spark_address);

    Ok(Json(WatchSparkDepositResponse {
        verifier_response: confirmation_status,
    }))
}