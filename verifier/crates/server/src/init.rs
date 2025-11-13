use crate::handlers;
use crate::middleware::build_signature;
use axum::routing::{get, post};
use axum::{Router, middleware};
use frost::signer::FrostSigner;
use std::sync::Arc;
use tracing::instrument;
use verifier_btc_indexer_client::client::BtcIndexerClient;
use verifier_config_parser::config::GatewayConfig;
use verifier_config_parser::config::ServerConfig;
use verifier_config_parser::config::{BtcIndexerConfig, SparkBalanceCheckerConfig};
use verifier_gateway_client::client::GatewayClient;
use verifier_local_db_store::storage::LocalDbStorage;
use verifier_spark_balance_checker_client::client::SparkBalanceCheckerClient;

#[derive(Clone)]
pub struct AppState {
    pub frost_signer: FrostSigner,
    pub btc_indexer_client: BtcIndexerClient,
    pub spark_balance_checker_client: SparkBalanceCheckerClient,
    pub gateway_client: GatewayClient,
    pub storage: Arc<LocalDbStorage>,
    pub server_config: ServerConfig,
}

pub struct VerifierApi {}

impl VerifierApi {
    pub const WATCH_SPARK_DEPOSIT_ENDPOINT: &'static str = "/api/gateway/watch-spark-deposit";
    pub const NOTIFY_RUNES_DEPOSIT_ENDPOINT: &'static str = "/api/btc-indexer/notify-runes-deposit";
    pub const WATCH_RUNES_DEPOSIT_ENDPOINT: &'static str = "/api/gateway/watch-runes-deposit";
    pub const DKG_ROUND1_ENDPOINT: &'static str = "/api/gateway/dkg-round-1";
    pub const DKG_ROUND2_ENDPOINT: &'static str = "/api/gateway/dkg-round-2";
    pub const DKG_FINALIZE_ENDPOINT: &'static str = "/api/gateway/dkg-finalize";
    pub const SIGN_ROUND1_ENDPOINT: &'static str = "/api/gateway/sign-round-1";
    pub const SIGN_ROUND2_ENDPOINT: &'static str = "/api/gateway/sign-round-2";
    pub const HEALTHCHECK_ENDPOINT: &'static str = "/health";
}

#[instrument(level = "trace", skip_all)]
pub async fn create_app(
    frost_signer: FrostSigner,
    btc_indexer_config: BtcIndexerConfig,
    spark_balance_checker_config: SparkBalanceCheckerConfig,
    gateway_client: GatewayConfig,
    storage: Arc<LocalDbStorage>,
    server_config: ServerConfig,
) -> Router {
    tracing::info!("Creating app");
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
            VerifierApi::WATCH_SPARK_DEPOSIT_ENDPOINT,
            post(handlers::watch_spark_deposit::handle),
        )
        .route(
            VerifierApi::NOTIFY_RUNES_DEPOSIT_ENDPOINT,
            post(handlers::notify_runes_deposit::handle),
        )
        .route(
            VerifierApi::WATCH_RUNES_DEPOSIT_ENDPOINT,
            post(handlers::watch_runes_deposit::handle),
        )
        .route(VerifierApi::DKG_ROUND1_ENDPOINT, post(handlers::dkg_round_1::handle))
        .route(VerifierApi::DKG_ROUND2_ENDPOINT, post(handlers::dkg_round_2::handle))
        .route(VerifierApi::DKG_FINALIZE_ENDPOINT, post(handlers::dkg_finalize::handle))
        .route(VerifierApi::SIGN_ROUND1_ENDPOINT, post(handlers::sign_round_1::handle))
        .route(VerifierApi::SIGN_ROUND2_ENDPOINT, post(handlers::sign_round_2::handle))
        .route(VerifierApi::HEALTHCHECK_ENDPOINT, get(handlers::healthcheck::handle))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state.clone(), build_signature))
}
