use bitcoin::Network;
use crate::error::FlowProcessorError;
use tracing;
use frost::types::MusigId;
use frost::aggregator::FrostAggregator;

pub async fn handle(
    musig_id: MusigId,
    amount: u64,
    network: Network,
    frost_aggregator: FrostAggregator,
) -> Result<String, FlowProcessorError> {

    Ok("".to_string())
}
