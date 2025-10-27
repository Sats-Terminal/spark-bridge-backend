use crate::error::ServerError;
use crate::handlers;
use axum::{
    Router,
    routing::{get, post},
};
use spark_client::client::SparkRpcClient;
use spark_client::common::config::SparkConfig;
use tracing::instrument;

#[derive(Clone)]
pub struct AppState {
    pub client: SparkRpcClient,
}

pub struct SparkBalanceCheckerApi;

impl SparkBalanceCheckerApi {
    /// Represents hardcoded `/track_tx` endpoint
    pub const VERIFY_BALANCE_ENDPOINT: &'static str = "/verify-balance";
    pub const HEALTHCHECK_ENDPOINT: &'static str = "/health";
}

#[instrument(level = "debug", ret, skip(config), fields(operators=?config.operators))]
pub async fn create_app(config: SparkConfig) -> Result<Router, ServerError> {
    tracing::info!("Creating app");
    let state = AppState {
        client: SparkRpcClient::new(config).await?,
    };
    let app = Router::new()
        .route(
            SparkBalanceCheckerApi::VERIFY_BALANCE_ENDPOINT,
            post(handlers::verify_balance::handle),
        )
        .route(
            SparkBalanceCheckerApi::HEALTHCHECK_ENDPOINT,
            get(handlers::healthcheck::handle),
        )
        .with_state(state);

    Ok(app)
}
