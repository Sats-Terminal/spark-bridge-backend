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
        .route("/api/user/runes-address", post(handlers::get_runes_address::handle))
        .route("/api/user/bridge-runes", post(handlers::bridge_runes::handle))
        .route("/api/user/exit-spark", post(handlers::exit_spark::handle))
        .route(
            "/api/verifier/notify-runes-address",
            post(handlers::notify_runes_address::handle),
        )
        .route(
            "/api/verifier/notify-spark-address",
            post(handlers::notify_spark_address::handle),
        )
        .with_state(state))
}
