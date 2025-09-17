use crate::handlers;
use axum::Router;
use axum::routing::post;
use gateway_deposit_verification::aggregator::DepositVerificationAggregator;
use gateway_flow_processor::flow_sender::FlowSender;
use tracing::instrument;
use bitcoin::Network;

#[derive(Clone)]
pub struct AppState {
    pub flow_sender: FlowSender,
    pub deposit_verification_aggregator: DepositVerificationAggregator,
    pub network: Network,
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
            "/api/user/get-btc-deposit-address",
            post(handlers::get_btc_deposit_address::handle),
        )
        .route(
            "/api/user/get-spark-deposit-address",
            post(handlers::get_spark_deposit_address::handle),
        )
        .route("/api/user/exit-spark", post(handlers::exit_spark::handle))
        .route(
            "/api/verifier/notify-runes-deposit",
            post(handlers::notify_runes_deposit::handle),
        )
        .route(
            "/api/verifier/notify-spark-deposit",
            post(handlers::notify_spark_deposit::handle),
        )
        .route(
            "/api/user/bridge-runes",
            post(handlers::bridge_runes::handle),
        )
        .with_state(state)
}
