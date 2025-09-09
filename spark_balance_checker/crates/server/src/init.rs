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

#[derive(OpenApi)]
#[openapi(paths(handlers::get_balance::handle))]
struct ApiDoc;

#[instrument(level = "debug", ret, skip(config), fields(operators=?config.operators))]
pub async fn create_app(config: SparkConfig) -> Router {
    info!(
        "[spark_balance_checker] Creating app with obtained config: {:?}, {:02X?}",
        config.operators,
        hex::encode(config.ca_pem.as_ref())
    );
    let state = AppState {
        client: SparkRpcClient::new(config).await.unwrap(),
    };
    let app = Router::new()
        .route("/balance", post(handlers::get_balance::handle))
        .with_state(state);

    #[cfg(feature = "swagger")]
    let app = app.merge(SwaggerUi::new("/swagger-ui/").url("/api-docs/openapi.json", ApiDoc::openapi()));
    info!("[spark_balance_checker] Successfully created app], app: {app:?}");
    app
}
