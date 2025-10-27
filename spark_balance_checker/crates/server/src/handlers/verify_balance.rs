use crate::error::ServerError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use global_utils::conversion::spark_network_to_proto_network;
use serde::{Deserialize, Serialize};
use spark_address::decode_spark_address;
use spark_protos::spark::QueryTokenOutputsRequest;
use token_identifier::TokenIdentifier;
use tracing::instrument;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct VerifyBalanceRequest {
    request_id: Uuid,
    spark_address: String,
    token_identifier: TokenIdentifier,
    amount: u128,
}

#[derive(Debug, Serialize)]
pub enum DepositStatus {
    Confirmed,
    Failed,
}

#[derive(Debug, Serialize)]
pub enum VerificationErrorDetails {
    TokenIdentifierNotFound(String),
    BalanceMismatch(String),
}

impl ToString for VerificationErrorDetails {
    fn to_string(&self) -> String {
        match self {
            VerificationErrorDetails::TokenIdentifierNotFound(msg) => msg.clone(),
            VerificationErrorDetails::BalanceMismatch(msg) => msg.clone(),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct VerifyBalanceResponse {
    deposit_status: DepositStatus,
    error_details: Option<String>,
}

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(payload): Json<VerifyBalanceRequest>,
) -> Result<Json<VerifyBalanceResponse>, ServerError> {
    let address_data = decode_spark_address(&payload.spark_address)
        .map_err(|e| ServerError::InvalidData(format!("Failed to decode spark address: {}", e)))?;
    let identity_public_key = hex::decode(address_data.identity_public_key)
        .map_err(|e| ServerError::InvalidData(format!("Failed to decode identity public key: {}", e)))?;

    let response = state
        .client
        .get_token_outputs(QueryTokenOutputsRequest {
            owner_public_keys: vec![identity_public_key],
            token_identifiers: vec![payload.token_identifier.to_bytes().to_vec()],
            token_public_keys: vec![],
            network: spark_network_to_proto_network(address_data.network) as i32,
        })
        .await;

    match response {
        Ok(response) => {
            let outputs = response.outputs_with_previous_transaction_data;
            if outputs.len() != 1 {
                tracing::error!(
                    "For request: {:?}, deposit address expects 1 output, got {}",
                    payload,
                    outputs.len()
                );
                Ok(Json(VerifyBalanceResponse {
                    deposit_status: DepositStatus::Failed,
                    error_details: Some(VerificationErrorDetails::TokenIdentifierNotFound("Expected 1 output, got {}".to_string()).to_string()),
                }))
            } else {
                let output = outputs.into_iter().next().unwrap();
                match output.output {
                    Some(output) => {
                        let encoded_balance = output.token_amount;

                        // Safe to unwrap because it is always encoded as u128
                        let balance = u128::from_be_bytes(encoded_balance.try_into().unwrap());
                        
                        if balance != payload.amount {
                            tracing::error!(
                                "For request: {:?}, balance mismatch, expected: {}, got {}",
                                payload,
                                payload.amount,
                                balance
                            );
                            Ok(Json(VerifyBalanceResponse {
                                deposit_status: DepositStatus::Failed,
                                error_details: Some(VerificationErrorDetails::BalanceMismatch("Balance mismatch".to_string()).to_string()),
                            }))
                        } else {
                            Ok(Json(VerifyBalanceResponse {
                                deposit_status: DepositStatus::Confirmed,
                                error_details: None,
                            }))
                        }
                    }
                    None => {
                        tracing::error!("For request: {:?}, output is None", payload);
                        Ok(Json(VerifyBalanceResponse {
                            deposit_status: DepositStatus::Failed,
                            error_details: Some(VerificationErrorDetails::TokenIdentifierNotFound("Output is None".to_string()).to_string()),
                        }))
                    }
                }
            }
        }
        Err(e) => Err(ServerError::from(e)),
    }
}
