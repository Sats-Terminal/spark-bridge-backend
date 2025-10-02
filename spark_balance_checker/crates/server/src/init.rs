use crate::handlers;
use axum::{Router, routing::post};
use spark_client::client::SparkRpcClient;
use spark_client::common::config::SparkConfig;
use tracing::{info, instrument};
use utoipa::OpenApi;

#[derive(Clone)]
pub struct AppState {
    pub client: SparkRpcClient,
}

#[allow(dead_code)]
#[derive(OpenApi)]
#[openapi(paths(handlers::get_balance::handle))]
struct ApiDoc;

pub struct SparkBalanceCheckerApi;

impl SparkBalanceCheckerApi {
    /// Represents hardcoded `/track_tx` endpoint
    pub const GET_BALANCE_ENDPOINT: &'static str = "/balance";
    pub const HEALTHCHECK_ENDPOINT: &'static str = "/health";
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
        .route(
            SparkBalanceCheckerApi::GET_BALANCE_ENDPOINT,
            post(handlers::get_balance::handle),
        )
        .route(
            SparkBalanceCheckerApi::HEALTHCHECK_ENDPOINT,
            post(handlers::healthcheck::handle),
        )
        .with_state(state);

    #[cfg(feature = "swagger")]
    let app = app.merge(SwaggerUi::new("/swagger-ui/").url("/api-docs/openapi.json", ApiDoc::openapi()));
    info!("[spark_balance_checker] Successfully created app], app: {app:?}");
    app
}
