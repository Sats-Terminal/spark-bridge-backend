use crate::errors::FlowProcessorError;
use tokio::sync::oneshot;
use uuid::Uuid;

pub type OneshotFlowProcessorSender = oneshot::Sender<Result<FlowProcessorResponse, FlowProcessorError>>;
pub type OneshotFlowProcessorReceiver = oneshot::Receiver<Result<FlowProcessorResponse, FlowProcessorError>>;

pub enum FlowProcessorMessage {
    RunDkgFlow(DkgFlowRequest),
    BridgeRunes(BridgeRunesRequest),
    ExitSpark(ExitSparkRequest),
    Testing(TestingRequest),
}

pub enum FlowProcessorResponse {
    RunDkgFlow(DkgFlowResponse),
    BridgeRunes(BridgeRunesResponse),
    ExitSpark(ExitSparkResponse),
    Testing(TestingResponse),
}

pub struct DkgFlowRequest {
    pub request_id: Uuid,
}

pub struct DkgFlowResponse {
    pub public_key: String,
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

pub struct TestingRequest {
    pub message: String,
    pub n_seconds: u64,
    pub n_runs: u64,
}

pub struct TestingResponse {
    pub message: String,
}
