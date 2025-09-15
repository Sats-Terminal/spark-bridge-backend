use crate::handlers;
use axum::Router;
use axum::routing::post;
use tracing::instrument;

use gateway_flow_processor::flow_sender::FlowSender;

#[derive(Clone)]
pub struct AppState {
    pub flow_sender: FlowSender,
}

#[instrument(level = "debug", skip(flow_sender), ret)]
pub async fn create_public_app(flow_sender: FlowSender) -> anyhow::Result<Router> {
    let state = AppState { flow_sender };
    Ok(Router::new()
        .route("/api/user/get-btc-deposit-address", post(handlers::get_btc_deposit_address::handle))
        .route("/api/user/get-spark-deposit-address", post(handlers::get_spark_deposit_address::handle))
        .route("/api/user/bridge-runes", post(handlers::bridge_runes::handle))
        .route("/api/user/exit-spark", post(handlers::exit_spark::handle))
        .route(
            "/api/verifier/notify-runes-address",
            post(handlers::notify_runes_address::handle),
        )
        .route(
            "/api/verifier/notify-spark-address",
            post(handlers::notify_spark_address::handle),
        )
        .with_state(state))
}
