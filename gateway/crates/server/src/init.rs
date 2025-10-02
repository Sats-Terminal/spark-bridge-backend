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

#[instrument(level = "trace", skip_all)]
pub async fn create_app(
    flow_sender: FlowSender,
    deposit_verification_aggregator: DepositVerificationAggregator,
    network: Network,
) -> Router {
    tracing::info!("Creating app");
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
        .route("/api/user/bridge-runes", post(handlers::bridge_runes::handle))
        .with_state(state)
}
