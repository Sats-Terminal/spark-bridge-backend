use axum::{
    Router,
    extract::{Json, State},
    routing::post,
};
use serde::{Deserialize, Serialize};
use spark_balance_checker_common::config::Config;
use spark_balance_checker_common::error::ServerError;
use spark_client::client::SparkRpcClient;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

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

#[derive(Deserialize, ToSchema)]
#[schema(example = json!({ 
    "spark_address": "sprt1pgss8fxt9jxuv4dgjwrg539s6u06ueausq076xvfej7wdah0htvjlxunt9fa4n", 
    "rune_id": "btknrt1p2sy7a8cx5pqfm3u4p2qfqa475fgwj3eg5d03hhk47t66605zf6qg52vj2" 
}))]
struct GetBalanceRequest {
    spark_address: String,
    rune_id: String,
}

#[derive(Serialize, ToSchema)]
#[schema(example = json!({ "balance": 1000 }))]
struct GetBalanceResponse {
    balance: u128,
}

#[utoipa::path(
    post,
    path = "/balance",
    request_body = GetBalanceRequest,
    responses(
        (status = 200, description = "Success", body = GetBalanceResponse),
        (status = 400, description = "Bad Request", body = String),
        (status = 500, description = "Internal Server Error", body = String),
    ),
)]
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
        Err(e) => Err(ServerError::from(e)),
    }
}

#[derive(OpenApi)]
#[openapi(paths(get_balance))]
struct ApiDoc;

pub async fn create_app(config: &Config) -> Router {
    let state = AppState::new(config);
    let app = Router::new().route("/balance", post(get_balance)).with_state(state);

    #[cfg(feature = "swagger")]
    let app = app.merge(SwaggerUi::new("/swagger-ui/").url("/api-docs/openapi.json", ApiDoc::openapi()));

    app
}
