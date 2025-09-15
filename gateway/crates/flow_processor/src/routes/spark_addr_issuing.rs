use crate::error::FlowProcessorError;
use crate::types::{IssueSparkDepositAddressRequest, IssueSparkDepositAddressResponse};
use bitcoin::Network;
use frost::types::MusigId;
use tracing;

pub async fn handle(musig_id: MusigId, amount: u64, network: Network) -> Result<String, FlowProcessorError> {
    Ok("".to_string())
}
