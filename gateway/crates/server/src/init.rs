use crate::{handlers, private_handlers};
use axum::Router;
use axum::routing::post;
use btc_resp_aggregator::aggregator::BtcConfirmationsAggregator;
use gateway_flow_processor::flow_sender::FlowSender;
use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, LazyLock};
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub flow_sender: FlowSender,
    pub btc_resp_checker: Arc<BtcConfirmationsAggregator>,
    pub private_api_socket_addr: SocketAddr,
}

#[instrument(level = "debug", skip(flow_sender, btc_resp_checker_shared), ret)]
pub async fn create_public_app(
    flow_sender: FlowSender,
    btc_resp_checker_shared: Arc<BtcConfirmationsAggregator>,
    addr_to_listen_private: (IpAddr, u16),
) -> anyhow::Result<Router> {
    let state = AppState {
        flow_sender,
        btc_resp_checker: btc_resp_checker_shared,
        private_api_socket_addr: SocketAddr::from(addr_to_listen_private),
    };
    Ok(Router::new()
        .route(
            "/api/user/get-btc-deposit-address",
            post(handlers::get_btc_deposit_address::handle),
        )
        .route(
            "/api/user/get-spark-deposit-address",
            post(handlers::get_spark_deposit_address::handle),
        )
        .route(
            BtcConfirmationsAggregator::RUN_BRIDGE_RUNE_SPARK_FLOW_PATH,
            post(handlers::bridge_runes::handle),
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
        .with_state(state))
}

#[derive(Clone)]
pub struct PrivateAppState {
    pub flow_sender: FlowSender,
    pub btc_resp_checker: Arc<BtcConfirmationsAggregator>,
}

#[instrument(level = "debug", skip(flow_sender, btc_confirmations_aggregator_shared), ret)]
pub async fn create_private_app(
    flow_sender: FlowSender,
    btc_confirmations_aggregator_shared: Arc<BtcConfirmationsAggregator>,
) -> anyhow::Result<Router> {
    let state = PrivateAppState {
        flow_sender,
        btc_resp_checker: btc_confirmations_aggregator_shared,
    };
    Ok(Router::new()
        .route(
            BtcConfirmationsAggregator::LOOPBACK_ENDPOINT_PATH,
            post(private_handlers::loopback_indexer_response::handle),
        )
        .with_state(state))
}
