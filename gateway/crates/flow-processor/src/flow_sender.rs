use tokio::sync::mpsc;
use tokio::sync::oneshot;
use crate::types::*;
use crate::errors::FlowProcessorError;
use tokio_util::sync::CancellationToken;

pub trait TypedMessageSender<S, R> {
    async fn send(&self, message: S) -> Result<R, FlowProcessorError>;
}

#[derive(Clone)]
pub struct FlowSender {
    tx_sender: mpsc::Sender<(FlowProcessorMessage, OneshotFlowProcessorSender)>,
    cancellation_token: CancellationToken,
}

impl FlowSender {
    pub fn new(
        tx_sender: mpsc::Sender<(FlowProcessorMessage, OneshotFlowProcessorSender)>,
        cancellation_token: CancellationToken
    ) -> Self {
        Self { tx_sender, cancellation_token }
    }

    pub async fn send_messsage(&self, message: FlowProcessorMessage) -> Result<FlowProcessorResponse, FlowProcessorError> {
        let (router_sender, router_receiver) = oneshot::channel::<Result<FlowProcessorResponse, FlowProcessorError>>();
        let send_response = self.tx_sender.send((message, router_sender)).await;
        
        match send_response {
            Ok(_) => {
                router_receiver
                    .await
                    .map_err(|_| FlowProcessorError::ChannelClosedError("Channel closed".to_string()))?
            }
            Err(e) => Err(FlowProcessorError::ChannelClosedError(e.to_string())),
        }
    }
    
    pub async fn shutdown(&self) {
        self.cancellation_token.cancel();
    }
}

impl TypedMessageSender<DkgFlowRequest, DkgFlowResponse> for FlowSender {
    async fn send(&self, dkg_message: DkgFlowRequest) -> Result<DkgFlowResponse, FlowProcessorError> {
        let response = self.send_messsage(FlowProcessorMessage::RunDkgFlow(dkg_message)).await?;
        match response {
            FlowProcessorResponse::RunDkgFlow(response) => Ok(response),
            _ => Err(FlowProcessorError::InvalidResponseType("Invalid response type".to_string())),
        }
    }
}

impl TypedMessageSender<BridgeRunesRequest, BridgeRunesResponse> for FlowSender {
    async fn send(&self, bridge_runes_message: BridgeRunesRequest) -> Result<BridgeRunesResponse, FlowProcessorError> {
        let response = self.send_messsage(FlowProcessorMessage::BridgeRunes(bridge_runes_message)).await?;
        match response {
            FlowProcessorResponse::BridgeRunes(response) => Ok(response),
            _ => Err(FlowProcessorError::InvalidResponseType("Invalid response type".to_string())),
        }
    }
}

impl TypedMessageSender<ExitSparkRequest, ExitSparkResponse> for FlowSender {
    async fn send(&self, exit_spark_message: ExitSparkRequest) -> Result<ExitSparkResponse, FlowProcessorError> {
        let response = self.send_messsage(FlowProcessorMessage::ExitSpark(exit_spark_message)).await?;
        match response {
            FlowProcessorResponse::ExitSpark(response) => Ok(response),
            _ => Err(FlowProcessorError::InvalidResponseType("Invalid response type".to_string())),
        }
    }
}

impl TypedMessageSender<TestingRequest, TestingResponse> for FlowSender {
    async fn send(&self, testing_message: TestingRequest) -> Result<TestingResponse, FlowProcessorError> {
        let response = self.send_messsage(FlowProcessorMessage::Testing(testing_message)).await?;
        match response {
            FlowProcessorResponse::Testing(response) => Ok(response),
            _ => Err(FlowProcessorError::InvalidResponseType("Invalid response type".to_string())),
        }
    }
}