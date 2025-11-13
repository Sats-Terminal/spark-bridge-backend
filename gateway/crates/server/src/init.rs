use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};
use bitcoin::Network;
use gateway_config_parser::config::{FeeConfig, VerifiersConfig};
use gateway_deposit_verification::aggregator::DepositVerificationAggregator;
use gateway_dkg_pregen::dkg_pregen_thread::DkgPregenThread;
use gateway_flow_processor::flow_sender::FlowSender;
use gateway_verifier_client::client::VerifierClient;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::instrument;

use crate::handlers;

#[derive(Clone)]
pub struct AppState {
    pub flow_sender: FlowSender,
    pub deposit_verification_aggregator: Arc<DepositVerificationAggregator>,
    pub network: Network,
    pub thread: TaskTracker,
    pub _dkg_pregen_thread: Arc<DkgPregenThread>,
    pub cancellation_token: CancellationToken,
    pub verifier_clients: Arc<Vec<VerifierClient>>,
    pub fee_cfg: Option<FeeConfig>,
}

pub struct GatewayApi;

impl GatewayApi {
    pub const GET_BTC_DEPOSIT_ADDRESS_ENDPOINT: &'static str = "/api/user/get-btc-deposit-address";
    pub const GET_SPARK_DEPOSIT_ADDRESS_ADDRESS_ENDPOINT: &'static str = "/api/user/get-spark-deposit-address";
    pub const EXIT_SPARK_ADDRESS_ENDPOINT: &'static str = "/api/user/exit-spark";
    pub const NOTIFY_RUNES_DEPOSIT_ADDRESS_ENDPOINT: &'static str = "/api/verifier/notify-runes-deposit";
    pub const NOTIFY_SPARK_DEPOSIT_ADDRESS_ENDPOINT: &'static str = "/api/verifier/notify-spark-deposit";
    pub const BRIDGE_RUNES_ADDRESS_ENDPOINT: &'static str = "/api/user/bridge-runes";
    pub const TEST_SPARK_ADDRESS_ENDPOINT: &'static str = "/api/test/test-spark";
    pub const HEALTHCHECK_ENDPOINT: &'static str = "/health";
}

#[instrument(level = "trace", skip_all)]
pub async fn create_app(
    flow_sender: FlowSender,
    deposit_verification_aggregator: DepositVerificationAggregator,
    network: Network,
    task_tracker: TaskTracker,
    dkg_pregen_thread: DkgPregenThread,
    verifiers_config: VerifiersConfig,
    fee_cfg: Option<FeeConfig>,
) -> Router {
    let cancellation_token = CancellationToken::new();
    let deposit_verification_aggregator = Arc::new(deposit_verification_aggregator);

    let verifier_clients = verifiers_config
        .0
        .iter()
        .map(|v| VerifierClient::new(v.clone()))
        .collect();
    let state = AppState {
        network,
        flow_sender,
        deposit_verification_aggregator,
        thread: task_tracker,
        _dkg_pregen_thread: Arc::new(dkg_pregen_thread),
        cancellation_token,
        verifier_clients: Arc::new(verifier_clients),
        fee_cfg,
    };
    Router::new()
        .route(
            GatewayApi::GET_BTC_DEPOSIT_ADDRESS_ENDPOINT,
            post(handlers::get_btc_deposit_address::handle),
        )
        .route(
            GatewayApi::GET_SPARK_DEPOSIT_ADDRESS_ADDRESS_ENDPOINT,
            post(handlers::get_spark_deposit_address::handle),
        )
        .route(
            GatewayApi::EXIT_SPARK_ADDRESS_ENDPOINT,
            post(handlers::exit_spark::handle),
        )
        .route(
            GatewayApi::NOTIFY_RUNES_DEPOSIT_ADDRESS_ENDPOINT,
            post(handlers::notify_runes_deposit::handle),
        )
        .route(
            GatewayApi::NOTIFY_SPARK_DEPOSIT_ADDRESS_ENDPOINT,
            post(handlers::notify_spark_deposit::handle),
        )
        .route(
            GatewayApi::BRIDGE_RUNES_ADDRESS_ENDPOINT,
            post(handlers::bridge_runes::handle),
        )
        .route(GatewayApi::HEALTHCHECK_ENDPOINT, get(handlers::healthcheck::handle))
        .with_state(state)
}
