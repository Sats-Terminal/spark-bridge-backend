use crate::error::ServerError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use spark_address::decode_spark_address;
use spark_protos::spark::QueryTokenOutputsRequest;
use token_identifier::TokenIdentifier;

#[derive(Deserialize)]
pub struct GetBalanceRequest {
    spark_address: String,
    token_identifier: TokenIdentifier,
}

#[derive(Serialize)]
pub struct GetBalanceResponse {
    balance: u128,
}

pub async fn handle(
    State(state): State<AppState>,
    Json(payload): Json<GetBalanceRequest>,
) -> Result<Json<GetBalanceResponse>, ServerError> {
    let address_data = decode_spark_address(&payload.spark_address)
        .map_err(|e| ServerError::InvalidData(format!("Failed to decode spark address: {}", e)))?;
    let identity_public_key = hex::decode(address_data.identity_public_key)
        .map_err(|e| ServerError::InvalidData(format!("Failed to decode identity public key: {}", e)))?;
    let network = address_data.network;

    let response = state
        .client
        .get_token_outputs(QueryTokenOutputsRequest {
            owner_public_keys: vec![identity_public_key],
            token_identifiers: vec![payload.token_identifier.to_bytes().to_vec()],
            token_public_keys: vec![],
            network: network as i32,
        })
        .await;

    match response {
        Ok(response) => {
            let outputs = response.outputs_with_previous_transaction_data;
            // TODO: I am not sure if this is correct.
            if outputs.len() != 1 {
                Err(ServerError::InvalidData(format!(
                    "Expected 1 output, got {}",
                    outputs.len()
                )))
            } else {
                // safe to unwrap because we know there is only one output
                let output = outputs.into_iter().next().unwrap();
                match output.output {
                    Some(output) => {
                        let encoded_balance = output.token_amount;
                        // safe to unwrap because we know the balance is 128 bits
                        let balance = u128::from_be_bytes(encoded_balance.try_into().unwrap());
                        Ok(Json(GetBalanceResponse { balance }))
                    }
                    None => Err(ServerError::InvalidData("Output is None".to_string())),
                }
            }
        }
        Err(e) => Err(ServerError::from(e)),
    }
}
