pub mod errors;
pub mod flow_processor;
pub mod flow_router;
pub mod flow_sender;
pub mod types;

use crate::flow_processor::FlowProcessor;
use crate::flow_sender::FlowSender;
use frost::aggregator::FrostAggregator;

use persistent_storage::init::PostgresRepo;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub fn create_flow_processor(
    storage: PostgresRepo,
    cancellation_retries: u64,
    frost_aggregator: FrostAggregator,
) -> (FlowProcessor, FlowSender) {
    let (tx_sender, tx_receiver) = mpsc::channel(1000);
    let cancellation_token = CancellationToken::new();
    let flow_processor = FlowProcessor::new(tx_receiver, storage, cancellation_token.clone(), cancellation_retries, frost_aggregator);
    let flow_sender = FlowSender::new(tx_sender, cancellation_token);
    (flow_processor, flow_sender)
}
