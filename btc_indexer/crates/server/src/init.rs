use axum::Router;
use axum::routing::get;
use crate::handlers;
use std::sync::Arc;
use btc_indexer_local_db_store::LocalDbStorage;
use btc_indexer_config::DatabaseConfig;
use bitcoin::Network;

#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<LocalDbStorage>,
    pub network: Network,
}

pub async fn create_app(
    network: Network,
    local_db_store: Arc<LocalDbStorage>,
) -> Router {
    let state = Arc::new(AppState {
        storage: local_db_store,
        network,
    });
    Router::new()
        .route("/health", get(handlers::healthcheck::handle))
        .with_state(state)
}
