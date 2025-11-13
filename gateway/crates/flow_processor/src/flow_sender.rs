use btc_indexer_client::client_api::IndexerClient;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{error::FlowProcessorError, types::*};

// This trait is used in order to send typed messages to the flow processor
#[async_trait::async_trait]
pub trait TypedMessageSender<S, R> {
    async fn send(&self, message: S) -> Result<R, FlowProcessorError>;
}

// This is helper struct that sends messages to the flow processor and waits for the response
// This struct implements the TypedMessageSender trait for each type of message
#[derive(Clone)]
pub struct FlowSender {
    pub btc_indexer: IndexerClient,
    tx_sender: mpsc::Sender<(FlowProcessorMessage, OneshotFlowProcessorSender)>,
    cancellation_token: CancellationToken,
}

impl FlowSender {
    pub fn new(
        btc_indexer: IndexerClient,
        tx_sender: mpsc::Sender<(FlowProcessorMessage, OneshotFlowProcessorSender)>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            btc_indexer,
            tx_sender,
            cancellation_token,
        }
    }

    pub async fn send_messsage(
        &self,
        message: FlowProcessorMessage,
    ) -> Result<FlowProcessorResponse, FlowProcessorError> {
        let (router_sender, router_receiver) = oneshot::channel::<Result<FlowProcessorResponse, FlowProcessorError>>();
        let send_response = self.tx_sender.send((message, router_sender)).await;

        match send_response {
            Ok(_) => router_receiver
                .await
                .map_err(|_| FlowProcessorError::ChannelClosedError("Channel closed".to_string()))?,
            Err(e) => Err(FlowProcessorError::ChannelClosedError(e.to_string())),
        }
    }

    pub async fn shutdown(&self) {
        self.cancellation_token.cancel();
    }
}

#[async_trait::async_trait]
impl TypedMessageSender<IssueBtcDepositAddressRequest, IssueBtcDepositAddressResponse> for FlowSender {
    async fn send(
        &self,
        dkg_message: IssueBtcDepositAddressRequest,
    ) -> Result<IssueBtcDepositAddressResponse, FlowProcessorError> {
        let response = self
            .send_messsage(FlowProcessorMessage::IssueBtcDepositAddress(dkg_message))
            .await?;
        match response {
            FlowProcessorResponse::IssueDepositAddress(response) => Ok(response),
            x => Err(FlowProcessorError::InvalidResponseType(format!(
                "Invalid response type, obtain: {x:?}, expected: [FlowProcessorResponse::RunDkgFlow]"
            ))),
        }
    }
}

#[async_trait::async_trait]
impl TypedMessageSender<IssueSparkDepositAddressRequest, IssueSparkDepositAddressResponse> for FlowSender {
    async fn send(
        &self,
        dkg_message: IssueSparkDepositAddressRequest,
    ) -> Result<IssueSparkDepositAddressResponse, FlowProcessorError> {
        let response = self
            .send_messsage(FlowProcessorMessage::IssueSparkDepositAddress(dkg_message))
            .await?;
        match response {
            FlowProcessorResponse::IssueSparkDepositAddress(response) => Ok(response),
            x => Err(FlowProcessorError::InvalidResponseType(format!(
                "Invalid response type, obtain: {x:?}, expected: [FlowProcessorResponse::IssueSparkDepositAddress]"
            ))),
        }
    }
}

#[async_trait::async_trait]
impl TypedMessageSender<BridgeRunesRequest, BridgeRunesResponse> for FlowSender {
    async fn send(&self, bridge_runes_message: BridgeRunesRequest) -> Result<BridgeRunesResponse, FlowProcessorError> {
        let response = self
            .send_messsage(FlowProcessorMessage::BridgeRunes(bridge_runes_message))
            .await?;
        match response {
            FlowProcessorResponse::BridgeRunes(response) => Ok(response),
            x => Err(FlowProcessorError::InvalidResponseType(format!(
                "Invalid response type, obtain: {x:?}, expected: [FlowProcessorResponse::BridgeRunes]"
            ))),
        }
    }
}

#[async_trait::async_trait]
impl TypedMessageSender<ExitSparkRequest, ExitSparkResponse> for FlowSender {
    async fn send(&self, exit_spark_message: ExitSparkRequest) -> Result<ExitSparkResponse, FlowProcessorError> {
        let response = self
            .send_messsage(FlowProcessorMessage::ExitSpark(exit_spark_message))
            .await?;
        match response {
            FlowProcessorResponse::ExitSpark(response) => Ok(response),
            x => Err(FlowProcessorError::InvalidResponseType(format!(
                "Invalid response type, obtain: {x:?}, expected: [FlowProcessorMessage::ExitSpark]"
            ))),
        }
    }
}
