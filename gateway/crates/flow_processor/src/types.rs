use crate::error::FlowProcessorError;
use bitcoin::Address;
use frost::types::MusigId;
use tokio::sync::oneshot;
use uuid::Uuid;

pub type OneshotFlowProcessorSender = oneshot::Sender<Result<FlowProcessorResponse, FlowProcessorError>>;
pub type OneshotFlowProcessorReceiver = oneshot::Receiver<Result<FlowProcessorResponse, FlowProcessorError>>;

pub enum FlowProcessorMessage {
    IssueBtcDepositAddress(IssueBtcDepositAddressRequest),
    IssueSparkDepositAddress(IssueSparkDepositAddressRequest),
    BridgeRunes(BridgeRunesRequest),
    ExitSpark(ExitSparkRequest),
}

#[derive(Debug)]
pub enum FlowProcessorResponse {
    IssueDepositAddress(IssueBtcDepositAddressResponse),
    IssueSparkDepositAddress(IssueSparkDepositAddressResponse),
    BridgeRunes(BridgeRunesResponse),
    ExitSpark(ExitSparkResponse),
}

#[derive(Debug)]
pub struct IssueBtcDepositAddressRequest {
    pub musig_id: MusigId,
    pub amount: u64,
}

#[derive(Debug)]
pub struct IssueBtcDepositAddressResponse {
    pub addr_to_replenish: Address,
}

#[derive(Debug)]
pub struct IssueSparkDepositAddressRequest {
    pub musig_id: MusigId,
    pub amount: u64,
}

#[derive(Debug)]
pub struct IssueSparkDepositAddressResponse {
    pub addr_to_replenish: String,
}

#[derive(Debug)]
pub struct BridgeRunesRequest {
    pub address: Address,
}

#[derive(Debug)]
pub struct BridgeRunesResponse {
    pub message: String,
}

#[derive(Debug)]
pub struct ExitSparkRequest {
    pub request_id: Uuid,
}

#[derive(Debug)]
pub struct ExitSparkResponse {
    pub message: String,
}
