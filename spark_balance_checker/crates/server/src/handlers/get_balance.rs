use crate::error::ServerError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use spark_address::decode_spark_address;
use spark_protos::spark::QueryTokenOutputsRequest;
use tracing::instrument;
use token_identifier::TokenIdentifier;
use global_utils::conversion::spark_network_to_proto_network;

#[derive(Deserialize, Debug)]
pub struct GetBalanceRequest {
    spark_address: String,
    token_identifier: TokenIdentifier,
}

#[derive(Serialize, Debug)]
pub struct GetBalanceResponse {
    balance: u128,
}

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(payload): Json<GetBalanceRequest>,
) -> Result<Json<GetBalanceResponse>, ServerError> {
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
                Err(ServerError::InvalidData(format!(
                    "Expected 1 output, got {}",
                    outputs.len()
                )))
            } else {
                let output = outputs.into_iter().next().unwrap();
                match output.output {
                    Some(output) => {
                        let encoded_balance = output.token_amount;

                        let balance = u128::from_be_bytes(encoded_balance.try_into().unwrap());
                        Ok(Json(GetBalanceResponse { balance }))
                    }
                    None => {
                        tracing::error!("For request: {:?}, output is None", payload);
                        Err(ServerError::InvalidData("Output is None".to_string()))
                    }
                }
            }
        }
        Err(e) => Err(ServerError::from(e)),
    }
}
