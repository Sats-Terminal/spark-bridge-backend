use crate::handlers;
use axum::{Router, routing::post};
use spark_client::SparkConfig;
use spark_client::client::SparkRpcClient;
use tracing::{info, instrument};
use utoipa::OpenApi;

#[derive(Clone)]
pub struct AppState {
    pub client: SparkRpcClient,
}

#[derive(OpenApi)]
#[openapi(paths(handlers::get_balance::handle))]
struct ApiDoc;

#[instrument(level = "debug", ret)]
pub async fn create_app(config: SparkConfig) -> Router {
    let state = AppState {
        client: SparkRpcClient::new(config),
    };
    let app = Router::new()
        .route("/balance", post(handlers::get_balance::handle))
        .with_state(state);

    #[cfg(feature = "swagger")]
    let app = app.merge(SwaggerUi::new("/swagger-ui/").url("/api-docs/openapi.json", ApiDoc::openapi()));
    info!("[spark_balance_checker] Successfully created app], app: {app:?}");
    app
}
