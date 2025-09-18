use crate::errors::VerifierError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use bitcoin::OutPoint;
use frost::types::MusigId;
use frost::types::Nonce;
use serde::{Deserialize, Serialize};
use verifier_local_db_store::schemas::deposit_address::DepositAddressStorage;
use verifier_local_db_store::schemas::deposit_address::{DepositStatus, DepositAddrInfo};
use verifier_spark_balance_checker_client::client::GetBalanceRequest;

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
    state
        .storage
        .set_deposit_addr_info(
            DepositAddrInfo {
                musig_id: request.musig_id.clone(),
                nonce: request.nonce,
                out_point: None,
                deposit_address: request.spark_address.clone(),
                bridge_address: request.exit_address,
                is_btc: true,
                amount: request.amount,
                confirmation_status: DepositStatus::WaitingForConfirmation,
            },
        )
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
        false => DepositStatus::Failed,
    };

    state.storage.set_confirmation_status_by_deposit_address(request.spark_address, confirmation_status.clone()).await.map_err(|e| VerifierError::StorageError(format!("Failed to update confirmation status: {}", e)))?;

    Ok(Json(WatchSparkDepositResponse {
        verifier_response: confirmation_status,
    }))
}
