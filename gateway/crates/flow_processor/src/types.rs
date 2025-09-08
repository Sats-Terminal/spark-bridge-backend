use crate::error::FlowProcessorError;
use bitcoin::secp256k1::PublicKey;
use frost::types::MusigId;
use tokio::sync::oneshot;
use uuid::Uuid;

pub type OneshotFlowProcessorSender = oneshot::Sender<Result<FlowProcessorResponse, FlowProcessorError>>;
pub type OneshotFlowProcessorReceiver = oneshot::Receiver<Result<FlowProcessorResponse, FlowProcessorError>>;

pub enum FlowProcessorMessage {
    RunDkgFlow(DkgFlowRequest),
    BridgeRunes(BridgeRunesRequest),
    ExitSpark(ExitSparkRequest),
}

#[derive(Debug)]
pub enum FlowProcessorResponse {
    RunDkgFlow(DkgFlowResponse),
    BridgeRunes(BridgeRunesResponse),
    ExitSpark(ExitSparkResponse),
}

#[derive(Debug)]
pub struct DkgFlowRequest {
    pub musig_id: MusigId,
}

#[derive(Debug)]
pub struct DkgFlowResponse {
    pub public_key: PublicKey,
}

#[derive(Debug)]
pub struct BridgeRunesRequest {
    pub request_id: Uuid,
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
