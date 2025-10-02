use crate::handlers;
use axum::{Router, routing::post};
use spark_client::client::SparkRpcClient;
use spark_client::common::config::SparkConfig;
use tracing::instrument;
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
    tracing::info!("Creating app");
    let state = AppState {
        client: SparkRpcClient::new(config).await.unwrap(),
    };
    let app = Router::new()
        .route("/balance", post(handlers::get_balance::handle))
        .with_state(state);

    #[cfg(feature = "swagger")]
    let app = app.merge(SwaggerUi::new("/swagger-ui/").url("/api-docs/openapi.json", ApiDoc::openapi()));
    app
}
