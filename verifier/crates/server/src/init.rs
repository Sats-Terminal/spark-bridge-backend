use crate::handlers;
use axum::Router;
use axum::routing::post;
use persistent_storage::init::PersistentRepoShared;
use tracing::instrument;

#[derive(Clone)]
pub struct AppState {
    db_pool: PersistentRepoShared,
}
#[instrument(level = "debug", skip(db_pool), ret)]
pub async fn create_app(db_pool: PersistentRepoShared) -> anyhow::Result<Router> {
    let state = AppState { db_pool };
    Ok(Router::new()
        .route(
            "/api/gateway/watch-spark-address",
            post(handlers::watch_spark_address::handle),
        )
        .route(
            "/api/gateway/watch-runes-address",
            post(handlers::watch_runes_address::handle),
        )
        .route(
            "/api/gateway/get-round-1-package",
            post(handlers::get_round_1_package::handle),
        )
        .route(
            "/api/gateway/get-round-2-package",
            post(handlers::get_round_2_package::handle),
        )
        .route(
            "/api/gateway/get-round-3-package",
            post(handlers::get_round_3_package::handle),
        )
        .with_state(state))
}
