use crate::handlers;
use axum::Router;
use axum::routing::post;
use frost::signer::FrostSigner;
use tracing::instrument;
use verifier_btc_indexer_client::client::BtcIndexerClient;
use verifier_spark_balance_checker_client::client::SparkBalanceCheckerClient;
use verifier_config_parser::config::{BtcIndexerConfig, SparkBalanceCheckerConfig, AppConfig};
use verifier_local_db_store::storage::LocalDbStorage;
use std::sync::Arc;
use verifier_gateway_client::client::GatewayClient;
use verifier_config_parser::config::GatewayConfig;
use verifier_config_parser::config::ServerConfig;

#[derive(Clone)]
pub struct AppState {
    pub frost_signer: FrostSigner,
    pub btc_indexer_client: BtcIndexerClient,
    pub spark_balance_checker_client: SparkBalanceCheckerClient,
    pub gateway_client: GatewayClient,
    pub storage: Arc<LocalDbStorage>,
    pub server_config: ServerConfig,
}

#[instrument(level = "debug", skip(frost_signer), ret)]
pub async fn create_app(
    frost_signer: FrostSigner,
    btc_indexer_config: BtcIndexerConfig,
    spark_balance_checker_config: SparkBalanceCheckerConfig,
    gateway_client: GatewayConfig,
    storage: Arc<LocalDbStorage>,
    server_config: ServerConfig,
) -> Router {
    let state = AppState {
        frost_signer,
        btc_indexer_client: BtcIndexerClient::new(btc_indexer_config),
        spark_balance_checker_client: SparkBalanceCheckerClient::new(spark_balance_checker_config),
        gateway_client: GatewayClient::new(gateway_client),
        storage,
        server_config,
    };
    Router::new()
        .route(
            "/api/gateway/watch-spark-deposit",
            post(handlers::watch_spark_deposit::handle),
        )
        .route(
            "/api/btc-indexer/notify-runes-deposit",
            post(handlers::notify_runes_deposit::handle),
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
