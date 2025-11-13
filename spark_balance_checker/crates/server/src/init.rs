use crate::error::ServerError;
use crate::handlers;
use axum::{
    Router,
    routing::{get, post},
};
use spark_balance_checker_config_parser::config::ServerConfig;
use spark_client::{client::SparkRpcClient, common::config::SparkConfig};
use sparkscan::client::SparkScanClient;
use tracing::instrument;

#[derive(Clone)]
pub struct AppState {
    pub client: SparkRpcClient,
    pub sparkscan: SparkScanClient,
    pub spark_config: SparkConfig,
}

pub const VERIFY_TRANSACTION_ENDPOINT: &'static str = "/verify-transaction";
pub const VERIFY_BALANCE_ENDPOINT: &'static str = "/verify-balance";
pub const HEALTHCHECK_ENDPOINT: &'static str = "/health";

#[instrument(level = "debug", ret, skip(config), fields(operators=?config.spark.operators))]
pub async fn create_app(config: ServerConfig) -> Result<Router, ServerError> {
    tracing::info!("Creating app");
    let state = AppState {
        client: SparkRpcClient::new(config.spark.clone()).await?,
        sparkscan: SparkScanClient::new(config.sparkscan.clone()),
        spark_config: config.spark,
    };
    let app = Router::new()
        .route(VERIFY_BALANCE_ENDPOINT, post(handlers::verify_balance::handle))
        .route(VERIFY_TRANSACTION_ENDPOINT, post(handlers::verify_transfer::handle))
        .route(HEALTHCHECK_ENDPOINT, get(handlers::healthcheck::handle))
        .with_state(state);

    Ok(app)
}
