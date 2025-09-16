use crate::flow_processor::FlowProcessor;
use crate::flow_sender::FlowSender;
use bitcoin::Network;
use frost::aggregator::FrostAggregator;
use gateway_local_db_store::storage::LocalDbStorage;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use gateway_config_parser::config::VerifierConfig;

pub fn create_flow_processor(
    verifier_configs: Arc<Vec<VerifierConfig>>,
    storage: Arc<LocalDbStorage>,
    cancellation_retries: u64,
    frost_aggregator: FrostAggregator,
    network: Network,
) -> (FlowProcessor, FlowSender) {
    let (tx_sender, tx_receiver) = mpsc::channel(1000);
    let cancellation_token = CancellationToken::new();
    let flow_processor = FlowProcessor::new(
        verifier_configs,
        tx_receiver,
        storage,
        cancellation_retries,
        frost_aggregator,
        network,
        cancellation_token.clone(),
    );
    let flow_sender = FlowSender::new(tx_sender, cancellation_token);
    (flow_processor, flow_sender)
}
