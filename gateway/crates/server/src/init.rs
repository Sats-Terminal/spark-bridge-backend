use crate::handlers;
use axum::Router;
use axum::routing::post;
use gateway_deposit_verification::aggregator::DepositVerificationAggregator;
use gateway_flow_processor::flow_sender::FlowSender;
use std::sync::Arc;
use tracing::instrument;
use crate::error::GatewayError;

#[derive(Clone)]
pub struct AppState {
    pub flow_sender: FlowSender,
    pub deposit_verification_aggregator: DepositVerificationAggregator,
}

#[instrument(level = "debug", skip(flow_sender), ret)]
pub async fn create_app(
    flow_sender: FlowSender,
    deposit_verification_aggregator: DepositVerificationAggregator,
) -> Router {
    let state = AppState {
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
            "/api/verifier/notify-runes-address",
            post(handlers::notify_runes_address::handle),
        )
        .route(
            "/api/verifier/notify-spark-address",
            post(handlers::notify_spark_address::handle),
        )
        .with_state(state)
}
