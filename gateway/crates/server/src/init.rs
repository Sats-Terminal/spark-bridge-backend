use crate::handlers;
use axum::Router;
use axum::routing::post;
use bitcoin::Network;
use gateway_deposit_verification::aggregator::DepositVerificationAggregator;
use gateway_flow_processor::flow_sender::FlowSender;
use tracing::instrument;

#[derive(Clone)]
pub struct AppState {
    pub flow_sender: FlowSender,
    pub deposit_verification_aggregator: DepositVerificationAggregator,
    pub network: Network,
}

pub struct GatewayApi;

impl GatewayApi {
    pub const GET_BTC_DEPOSIT_ADDRESS_ENDPOINT: &'static str = "/api/user/get-btc-deposit-address";
    pub const GET_SPARK_DEPOSIT_ADDRESS_ADDRESS_ENDPOINT: &'static str = "/api/user/get-spark-deposit-address";
    pub const EXIT_SPARK_ADDRESS_ENDPOINT: &'static str = "/api/user/exit-spark";
    pub const NOTIFY_RUNES_DEPOSIT_ADDRESS_ENDPOINT: &'static str = "/api/verifier/notify-runes-deposit";
    pub const BRIDGE_RUNES_ADDRESS_ENDPOINT: &'static str = "/api/user/bridge-runes";
    pub const HEALTHCHECK_ENDPOINT: &'static str = "/health";
}

#[instrument(level = "debug", skip(flow_sender), ret)]
pub async fn create_app(
    flow_sender: FlowSender,
    deposit_verification_aggregator: DepositVerificationAggregator,
    network: Network,
) -> Router {
    let state = AppState {
        network,
        flow_sender,
        deposit_verification_aggregator,
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
            GatewayApi::BRIDGE_RUNES_ADDRESS_ENDPOINT,
            post(handlers::bridge_runes::handle),
        )
        .route(GatewayApi::HEALTHCHECK_ENDPOINT, post(handlers::healthcheck::handle))
        .with_state(state)
}
