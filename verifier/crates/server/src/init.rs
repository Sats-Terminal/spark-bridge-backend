use crate::handlers;
use axum::Router;
use axum::routing::post;
use btc_resp_aggregator::tx_checker::BtcTxChecker;
use frost::signer::FrostSigner;
use tracing::instrument;

#[derive(Clone)]
pub struct AppState {
    pub frost_signer: FrostSigner,
    pub btc_tx_checker: BtcTxChecker,
}

#[instrument(level = "debug", skip(frost_signer, btc_tx_checker), ret)]
pub async fn create_app(frost_signer: FrostSigner, btc_tx_checker: BtcTxChecker) -> anyhow::Result<Router> {
    let state = AppState {
        frost_signer,
        btc_tx_checker,
    };
    Ok(Router::new()
        .route(
            "/api/gateway/watch-spark-address",
            post(handlers::watch_spark_address::handle),
        )
        .route(
            BtcTxChecker::LOOPBACK_ENDPOINT_PATH,
            post(handlers::loopback_btc_indexer_response::handle),
        )
        .route("/api/gateway/check-tx", post(handlers::check_tx::handle))
        .route(
            "/api/gateway/watch-runes-address",
            post(handlers::watch_runes_address::handle),
        )
        .route("/api/gateway/dkg-round-1", post(handlers::dkg_round_1::handle))
        .route("/api/gateway/dkg-round-2", post(handlers::dkg_round_2::handle))
        .route("/api/gateway/dkg-finalize", post(handlers::dkg_finalize::handle))
        .with_state(state))
}
