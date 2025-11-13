use axum::{Json, extract::State};
use spark_address::{Network, decode_spark_address};
use sparkscan::models::{Network as ScanNetwork, TxStatus};
use tracing::instrument;

use crate::{
    error::ServerError,
    init::AppState,
    models::{DepositStatus, VerifyBalanceResponse, VerifyTransferRequest},
};

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(payload): Json<VerifyTransferRequest>,
) -> Result<Json<VerifyBalanceResponse>, ServerError> {
    let address_data = decode_spark_address(&payload.receiver_address)
        .map_err(|e| ServerError::InvalidData(format!("Failed to decode spark address: {}", e)))?;

    let tx = state
        .sparkscan
        .get_transaction(
            &payload.txid,
            match address_data.network {
                Network::Mainnet => ScanNetwork::Mainnet,
                Network::Regtest => ScanNetwork::Regtest,
                _ => {
                    return Err(ServerError::InvalidData(format!(
                        "Unsupported network, {}",
                        &payload.receiver_address
                    )));
                }
            },
        )
        .await?;

    tracing::warn!("Sparkscan tx {:?}", tx);
    if tx.status == TxStatus::Failed || tx.status == TxStatus::Expired {
        return Ok(new_failed_verification_response("Invalid tx status"));
    }

    let transfer_amount = tx.amount_sats;
    if transfer_amount != payload.amount {
        return Ok(new_failed_verification_response(&format!(
            "Amount mismatch: expected {}, got {}",
            payload.amount, transfer_amount
        )));
    }

    match tx.to.pubkey {
        Some(pubkey) => {
            if pubkey != address_data.identity_public_key {
                return Ok(new_failed_verification_response(
                    "Receiver identity public key mismatch",
                ));
            }
        }
        None => return Ok(new_failed_verification_response("Missing receiver identity public key")),
    }

    Ok(Json(VerifyBalanceResponse {
        deposit_status: DepositStatus::Confirmed,
        error_details: None,
    }))
}

fn new_failed_verification_response(error_details: &str) -> Json<VerifyBalanceResponse> {
    Json(VerifyBalanceResponse {
        deposit_status: DepositStatus::Failed,
        error_details: Some(error_details.to_string()),
    })
}
