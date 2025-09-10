use crate::handlers;
use crate::state::AppState;
use axum::Router;
use axum::routing::post;
use frost::signer::FrostSigner;
use tracing::instrument;

#[instrument(level = "debug", skip(frost_signer), ret)]
pub async fn create_app(frost_signer: FrostSigner) -> anyhow::Result<Router> {
    let state = AppState { frost_signer };
    Ok(Router::new()
        .route(
            "/api/gateway/watch-spark-address",
            post(handlers::watch_spark_address::handle),
        )
        .route(
            "/api/gateway/watch-runes-address",
            post(handlers::watch_runes_address::handle),
        )
        .route("/api/gateway/dkg-round-1", post(handlers::dkg_round_1::handle))
        .route("/api/gateway/dkg-round-2", post(handlers::dkg_round_2::handle))
        .route("/api/gateway/dkg-finalize", post(handlers::dkg_finalize::handle))
        .with_state(state))
}
