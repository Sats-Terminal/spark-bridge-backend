use crate::handlers::dkg::{handler_get_round_1_package, handler_get_round_2_package, handler_get_round_3_package};
use crate::handlers::watchers::{handler_watch_runes_address, handler_watch_spark_address};
use axum::Router;
use axum::handler::Handler;
use axum::routing::post;
use tracing::instrument;
use verifier_local_db_store::init::LocalDbStorage;

#[derive(Clone)]
pub struct AppState {
    db_pool: LocalDbStorage,
}
#[instrument(level = "debug", ret)]
pub async fn create_app(db_pool: LocalDbStorage) -> anyhow::Result<Router> {
    let state = AppState { db_pool };
    Ok(Router::new()
        .route("/api/gateway/watch-spark-address", post(handler_watch_spark_address))
        .route("/api/gateway/watch-runes-address", post(handler_watch_runes_address))
        .route("/api/gateway/get-round-1-package", post(handler_get_round_1_package))
        .route("/api/gateway/get-round-2-package", post(handler_get_round_2_package))
        .route("/api/gateway/get-round-3-package", post(handler_get_round_3_package))
        .with_state(state))
}
