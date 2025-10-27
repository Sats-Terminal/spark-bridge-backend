use axum::Router;
use axum::routing::{get, post};
use crate::handlers;
use std::sync::Arc;
use btc_indexer_local_db_store::storage::LocalDbStorage;
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
    let state = AppState {
        storage: local_db_store,
        network,
    };
    Router::new()
        .route("/health", get(handlers::healthcheck::handle))
        .route("/watch", post(handlers::watch::handle))
        .with_state(state)
}
