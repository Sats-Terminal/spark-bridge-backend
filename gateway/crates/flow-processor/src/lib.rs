pub mod types;
pub mod flow_executor;
pub mod flow_sender;
pub mod errors;

use crate::flow_executor::FlowProcessor;
use crate::flow_sender::FlowSender;

use tokio::sync::mpsc;
use persistent_storage::init::PostgresRepo;

pub fn create_flow_processor(storage: PostgresRepo) -> (FlowProcessor, FlowSender) {
    let (tx_sender, tx_receiver) = mpsc::channel(1000);
    let flow_processor = FlowProcessor::new(tx_receiver, storage);
    let flow_sender = FlowSender::new(tx_sender);
    (flow_processor, flow_sender)
}