use crate::handlers;
use axum::{Router, routing::post};
use spark_client::client::SparkRpcClient;
use spark_client::common::config::SparkConfig;
use tracing::{info, instrument};

#[derive(Clone)]
pub struct AppState {
    pub client: SparkRpcClient,
}

#[instrument(level = "debug", ret, skip(config), fields(operators=?config.operators))]
pub async fn create_app(config: SparkConfig) -> Router {
    info!(
        "[spark_balance_checker] Creating app with obtained config: {:?}",
        config,
    );
    let state = AppState {
        client: SparkRpcClient::new(config).await.unwrap(),
    };
    let app = Router::new()
        .route("/balance", post(handlers::get_balance::handle))
        .with_state(state);

    info!("[spark_balance_checker] Successfully created app], app: {app:?}");
    app
}
