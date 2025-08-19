use axum::{
    Router,
    extract::{Json, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use spark_balance_checker_client::client::SparkRpcClient;
use spark_balance_checker_common::config::Config;
use spark_balance_checker_common::error::ServerError;

#[derive(Clone)]
struct AppState {
    client: SparkRpcClient,
}

impl AppState {
    fn new(config: &Config) -> Self {
        Self {
            client: SparkRpcClient::new(config.spark.clone()),
        }
    }
}

#[derive(Deserialize)]
struct GetBalanceRequest {
    spark_address: String,
    rune_id: String,
}

#[derive(Serialize)]
struct GetBalanceResponse {
    balance: u128,
}

async fn get_balance(
    State(mut state): State<AppState>,
    Json(payload): Json<GetBalanceRequest>,
) -> Result<Json<GetBalanceResponse>, ServerError> {
    let response = state
        .client
        .get_token_outputs(payload.spark_address, payload.rune_id)
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
        Err(e) => Err(e),
    }
}

pub async fn create_app(config: &Config) -> Router {
    let state = AppState::new(config);
    let app = Router::new().route("/balance", get(get_balance)).with_state(state);
    app
}
