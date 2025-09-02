use crate::handlers;
use axum::Router;
use axum::handler::Handler;
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
            "/api/gateway/dkg-round-1",
            post(handlers::dkg_round_1::handle),
        )
        .route(
            "/api/gateway/dkg-round-2",
            post(handlers::dkg_round_2::handle),
        )
        .route(
            "/api/gateway/dkg-finalize",
            post(handlers::dkg_finalize::handle),
        )
        .with_state(state))
}
