use crate::handlers;
use axum::Router;
use axum::routing::post;
use frost::signer::FrostSigner;
use tracing::instrument;

#[derive(Clone)]
pub struct AppState {
    pub frost_signer: FrostSigner,
}

#[instrument(level = "debug", skip(frost_signer), ret)]
pub async fn create_app(frost_signer: FrostSigner) -> Router {
    let state = AppState {
        frost_signer,
    };
    Router::new()
        .route(
            "/api/gateway/watch-spark-deposit",
            post(handlers::watch_spark_deposit::handle),
        )
        .route(
            "/api/btc-indexer/notify-runes-deposit",
            post(handlers::indexer_notify_deposit::handle),
        )
        .route(
            "/api/gateway/watch-runes-deposit",
            post(handlers::watch_runes_deposit::handle),
        )
        .route("/api/gateway/dkg-round-1", post(handlers::dkg_round_1::handle))
        .route("/api/gateway/dkg-round-2", post(handlers::dkg_round_2::handle))
        .route("/api/gateway/dkg-finalize", post(handlers::dkg_finalize::handle))
        .with_state(state)
}
