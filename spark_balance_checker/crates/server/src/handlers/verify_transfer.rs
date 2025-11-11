use std::{cmp::Ordering, str::FromStr};

use axum::Json;
use axum::extract::State;
use bitcoin::secp256k1::PublicKey;
use global_utils::conversion::spark_network_to_proto_network;
use spark_address::decode_spark_address;
use spark_protos::spark::{TransferFilter, TransferStatus, transfer_filter::Participant};
use tracing::instrument;

use crate::{
    error::ServerError,
    init::AppState,
    models::{DepositStatus, VerificationErrorDetails, VerifyBalanceResponse, VerifyTransferRequest},
};

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(payload): Json<VerifyTransferRequest>,
) -> Result<Json<VerifyBalanceResponse>, ServerError> {
    let address_data = decode_spark_address(&payload.receiver_address)
        .map_err(|e| ServerError::InvalidData(format!("Failed to decode spark address: {}", e)))?;
    let identity_public_key = hex::decode(address_data.identity_public_key)
        .map_err(|e| ServerError::InvalidData(format!("Failed to decode identity public key: {}", e)))?;

    let coordinator_config = state.spark_config.coordinator_operator_config()?;
    let coordinator_public_key = PublicKey::from_str(&coordinator_config.identity_public_key)
        .map_err(|e| ServerError::InvalidData(format!("Failed to decode coordinator identity public key: {}", e)))?;

    let response = state
        .client
        .get_transfers(
            TransferFilter {
                transfer_ids: vec![payload.txid.clone()],
                limit: 1,
                offset: 0,
                types: vec![],
                network: spark_network_to_proto_network(address_data.network) as i32,
                // 2, 3, 4, 5, 9, 10
                statuses: vec![
                    TransferStatus::SenderKeyTweaked as i32,
                    TransferStatus::ReceiverKeyTweaked as i32,
                    TransferStatus::ReceiverRefundSigned as i32,
                    TransferStatus::Completed as i32,
                    TransferStatus::ReceiverKeyTweakLocked as i32,
                    TransferStatus::ReceiverKeyTweakApplied as i32,
                ],
                order: 0,
                participant: None,
            },
            coordinator_public_key,
        )
        .await?;

    if response.transfers.len() != 1 {
        return Ok(Json(VerifyBalanceResponse {
            deposit_status: DepositStatus::Failed,
            error_details: Some(
                VerificationErrorDetails::InvalidTransfer(format!(
                    "Expected 1 transfer, got {}",
                    response.transfers.len(),
                ))
                .to_string(),
            ),
        }));
    }

    let transfer = response.transfers[0].clone();
    let transfer_amount = transfer.total_value;
    if transfer_amount != payload.amount {
        return Ok(Json(VerifyBalanceResponse {
            deposit_status: DepositStatus::Failed,
            error_details: Some(
                VerificationErrorDetails::InvalidTransfer(format!(
                    "Amount mismatch: expected {}, got {}",
                    payload.amount, transfer_amount
                ))
                .to_string(),
            ),
        }));
    }

    if transfer.receiver_identity_public_key.cmp(&identity_public_key) != Ordering::Equal {
        return Ok(Json(VerifyBalanceResponse {
            deposit_status: DepositStatus::Failed,
            error_details: Some(
                VerificationErrorDetails::InvalidTransfer(format!("Receiver identity public key mismatch",))
                    .to_string(),
            ),
        }));
    }

    Ok(Json(VerifyBalanceResponse {
        deposit_status: DepositStatus::Confirmed,
        error_details: None,
    }))
}
