use crate::error::FlowProcessorError;
use bitcoin::Address;
use tokio::sync::oneshot;
use gateway_local_db_store::schemas::user_identifier::UserId;

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
    pub user_id: UserId,
    pub rune_id: String,
    pub amount: u64,
}

#[derive(Debug)]
pub struct IssueBtcDepositAddressResponse {
    pub addr_to_replenish: Address,
}

#[derive(Debug)]
pub struct IssueSparkDepositAddressRequest {
    pub user_id: UserId,
    pub rune_id: String,
    pub amount: u64,
}

#[derive(Debug)]
pub struct IssueSparkDepositAddressResponse {
    pub addr_to_replenish: String,
}

#[derive(Debug)]
pub struct BridgeRunesRequest {
    pub btc_address: Address,
}

#[derive(Debug)]
pub struct BridgeRunesResponse {}

#[derive(Debug)]
pub struct ExitSparkRequest {
    pub spark_address: String,
}

#[derive(Debug)]
pub struct ExitSparkResponse {}
