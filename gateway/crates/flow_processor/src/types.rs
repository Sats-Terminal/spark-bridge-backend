use crate::errors::FlowProcessorError;
use tokio::sync::oneshot;
use frost::types::MusigId;
use bitcoin::secp256k1::PublicKey;
use uuid::Uuid;

pub type OneshotFlowProcessorSender = oneshot::Sender<Result<FlowProcessorResponse, FlowProcessorError>>;
pub type OneshotFlowProcessorReceiver = oneshot::Receiver<Result<FlowProcessorResponse, FlowProcessorError>>;

pub enum FlowProcessorMessage {
    RunDkgFlow(DkgFlowRequest),
    BridgeRunes(BridgeRunesRequest),
    ExitSpark(ExitSparkRequest),
}

pub enum FlowProcessorResponse {
    RunDkgFlow(DkgFlowResponse),
    BridgeRunes(BridgeRunesResponse),
    ExitSpark(ExitSparkResponse),
}

pub struct DkgFlowRequest {
    pub musig_id: MusigId,
}

pub struct DkgFlowResponse {
    pub public_key: PublicKey,
}

pub struct BridgeRunesRequest {
    pub request_id: Uuid,
}

pub struct BridgeRunesResponse {
    pub message: String,
}

pub struct ExitSparkRequest {
    pub request_id: Uuid,
}

pub struct ExitSparkResponse {
    pub message: String,
}
