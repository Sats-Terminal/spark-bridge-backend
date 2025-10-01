use crate::handlers;
use axum::Router;
use axum::routing::post;
use bitcoin::Network;
use frost::aggregator::FrostAggregator;
use gateway_config_parser::config::DkgPregenConfig;
use gateway_deposit_verification::aggregator::DepositVerificationAggregator;
use gateway_flow_processor::flow_sender::FlowSender;
use gateway_local_db_store::storage::LocalDbStorage;
use gateway_verifier_client::client::VerifierClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::instrument;

#[derive(Clone)]
pub struct AppState {
    pub flow_sender: FlowSender,
    pub deposit_verification_aggregator: Arc<DepositVerificationAggregator>,
    pub network: Network,
    pub typed_verifiers_clients: HashMap<u16, Arc<VerifierClient>>,
    pub thread: TaskTracker,
    pub cancellation_token: CancellationToken,
}

pub struct GatewayApi;

impl GatewayApi {
    /// Represents hardcoded `/track_tx` endpoint
    pub const GET_BTC_DEPOSIT_ADDRESS_ENDPOINT: &'static str = "/api/user/get-btc-deposit-address";
    pub const GET_SPARK_DEPOSIT_ADDRESS_ADDRESS_ENDPOINT: &'static str = "/api/user/get-spark-deposit-address";
    pub const EXIT_SPARK_ADDRESS_ENDPOINT: &'static str = "/api/user/exit-spark";
    pub const GET_TOKENS_BRIDGE_STATUS_ENDPOINT: &'static str = "/api/user/get_status";
    pub const NOTIFY_RUNES_DEPOSIT_ADDRESS_ENDPOINT: &'static str = "/api/verifier/notify-runes-deposit";
    pub const BRIDGE_RUNES_ADDRESS_ENDPOINT: &'static str = "/api/user/bridge-runes";
    pub const TEST_SPARK_ADDRESS_ENDPOINT: &'static str = "/api/test/test-spark";
    pub const HEALTHCHECK_ENDPOINT: &'static str = "/healthcheck";
}

#[instrument(level = "debug", skip(flow_sender, local_db, task_tracker), ret)]
pub async fn create_app(
    flow_sender: FlowSender,
    deposit_verification_aggregator: DepositVerificationAggregator,
    network: Network,
    typed_verifiers_clients: HashMap<u16, Arc<VerifierClient>>,
    local_db: Arc<LocalDbStorage>,
    frost_aggregator: Arc<FrostAggregator>,
    mut task_tracker: TaskTracker,
    dkg_pregen_config: DkgPregenConfig,
) -> Router {
    let cancellation_token = CancellationToken::new();
    let deposit_verification_aggregator = Arc::new(deposit_verification_aggregator);
    crate::dkg_pregen_thread::DkgPregenThread::spawn_thread(
        &mut task_tracker,
        local_db,
        dkg_pregen_config,
        frost_aggregator,
        cancellation_token.clone(),
    )
    .await;
    let state = AppState {
        network,
        flow_sender,
        deposit_verification_aggregator,
        typed_verifiers_clients,
        thread: task_tracker,
        cancellation_token,
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
            GatewayApi::GET_TOKENS_BRIDGE_STATUS_ENDPOINT,
            post(handlers::get_token_bridge_status::handle),
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
            GatewayApi::BRIDGE_RUNES_ADDRESS_ENDPOINT,
            post(handlers::bridge_runes::handle),
        )
        .route(
            GatewayApi::TEST_SPARK_ADDRESS_ENDPOINT,
            post(handlers::test_spark::handle),
        )
        .route(GatewayApi::HEALTHCHECK_ENDPOINT, post(handlers::healthcheck::handle))
        .with_state(state)
}
