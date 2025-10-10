use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use frost::types::TweakBytes;
use serde::{Deserialize, Serialize};
use token_identifier::TokenIdentifier;
use tracing;
use tracing::instrument;
use verifier_local_db_store::schemas::deposit_address::DepositAddressStorage;
use verifier_local_db_store::schemas::deposit_address::{DepositAddrInfo, DepositStatus, InnerAddress};
use verifier_local_db_store::schemas::user_identifier::{UserIdentifierStorage, UserIds};
use verifier_spark_balance_checker_client::client::GetBalanceRequest;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WatchSparkDepositRequest {
    pub user_ids: UserIds,
    pub nonce: TweakBytes,
    pub spark_address: String,
    pub exit_address: String,
    pub amount: u64,
    pub token_identifier: TokenIdentifier,
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

    let deposit_address = InnerAddress::SparkAddress(request.spark_address.clone());
    let bridge_address = InnerAddress::from_string_and_type(request.exit_address, true)
        .map_err(|e| VerifierError::Validation(format!("Invalid exit address: {}", e)))?;

    state
        .storage
        .insert_user_ids(request.user_ids.clone())
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set identifier data: {}", e)))?;

    state
        .storage
        .set_deposit_addr_info(DepositAddrInfo {
            dkg_share_id: request.user_ids.dkg_share_id,
            nonce: request.nonce,
            outpoint: None,
            deposit_address: deposit_address.clone(),
            bridge_address,
            is_btc: false,
            deposit_amount: request.amount,
            sats_fee_amount: None,
            confirmation_status: DepositStatus::WaitingForConfirmation,
        })
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to set deposit address info: {}", e)))?;

    tracing::info!("Getting balance for spark address: {}", request.spark_address);

    let response = state
        .spark_balance_checker_client
        .get_balance(GetBalanceRequest {
            spark_address: request.spark_address.clone(),
            // rune_id: request.user_unique_id.rune_id,
            token_identifier: request.token_identifier,
        })
        .await
        .map_err(|e| VerifierError::SparkBalanceCheckerClient(format!("Failed to get balance: {}", e)))?;

    tracing::info!("Balance: {:?}", response);

    let confirmation_status = match response.balance == request.amount as u128 {
        true => DepositStatus::Confirmed,
        false => {
            tracing::error!("Balance is not equal to amount for spark address: {}", request.spark_address);
            DepositStatus::Failed
        },
    };

    state
        .storage
        .set_confirmation_status_by_deposit_address(deposit_address, confirmation_status.clone())
        .await
        .map_err(|e| VerifierError::Storage(format!("Failed to update confirmation status: {}", e)))?;

    tracing::info!("Spark deposit watched for address: {}", request.spark_address);

    Ok(Json(WatchSparkDepositResponse {
        verifier_response: confirmation_status,
    }))
}
