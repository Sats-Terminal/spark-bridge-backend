use crate::error::FlowProcessorError;
use crate::types::*;
use bitcoin::Network;
use frost::aggregator::FrostAggregator;
use gateway_config_parser::config::VerifierConfig;
use gateway_local_db_store::storage::LocalDbStorage;
use gateway_rune_transfer::bitcoin_client::BitcoinClient;
use gateway_spark_service::service::SparkService;
use spark_client::client::SparkRpcClient;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing;
use tracing::info;
use uuid::Uuid;

const LOG_PATH: &str = "flow_processor";

// This struct is used to route the message to the correct flow
// This struct instance is created for each message that is sent to the flow processor
pub struct FlowProcessorRouter {
    pub verifier_configs: Arc<Vec<VerifierConfig>>,
    pub storage: Arc<LocalDbStorage>,
    pub flow_id: Uuid,
    pub response_sender: OneshotFlowProcessorSender,
    pub task_sender: mpsc::Sender<Uuid>,
    pub frost_aggregator: FrostAggregator,
    pub spark_service: Arc<SparkService>,
    pub spark_client: Arc<SparkRpcClient>,
    pub network: Network,
    pub bitcoin_client: Arc<BitcoinClient>,
}

impl FlowProcessorRouter {
    pub async fn run(mut self, message: FlowProcessorMessage) {
        #[allow(clippy::let_and_return)]
        let response = match message {
            FlowProcessorMessage::IssueBtcDepositAddress(request) => {
                let response = self.run_btc_addr_issuing(request, self.network).await;
                let answer = response.map(FlowProcessorResponse::IssueDepositAddress);
                answer
            }
            FlowProcessorMessage::IssueSparkDepositAddress(request) => {
                let response = self.run_spark_addr_issuing(request).await;
                let answer = response.map(FlowProcessorResponse::IssueSparkDepositAddress);
                answer
            }
            FlowProcessorMessage::BridgeRunes(request) => {
                let response = self.run_bridge_runes_flow(request).await;
                let answer = response.map(FlowProcessorResponse::BridgeRunes);
                answer
            }
            FlowProcessorMessage::ExitSpark(request) => {
                let response = self.run_exit_spark_flow(request).await;
                let answer = response.map(FlowProcessorResponse::ExitSpark);
                answer
            }
        };

        let _ = self.response_sender.send(response).map_err(|_| {
            tracing::error!("[router] Failed to send response for flow id {}", self.flow_id);
        });

        let _ = self.task_sender.send(self.flow_id).await.map_err(|_| {
            tracing::error!("[router] Failed to send task for flow id {}", self.flow_id);
        });
    }

    #[tracing::instrument(level = "trace", skip(self, request), ret)]
    async fn run_btc_addr_issuing(
        &mut self,
        request: IssueBtcDepositAddressRequest,
        network: Network,
    ) -> Result<IssueBtcDepositAddressResponse, FlowProcessorError> {
        info!("[{LOG_PATH}] issuing btc addr to user with request: {request:?}");
        let pubkey = crate::routes::btc_addr_issuing::handle(self, request).await?;
        Ok(IssueBtcDepositAddressResponse {
            addr_to_replenish: pubkey,
        })
    }

    #[tracing::instrument(level = "trace", skip(self, request), ret)]
    async fn run_bridge_runes_flow(
        &mut self,
        request: BridgeRunesRequest,
    ) -> Result<BridgeRunesResponse, FlowProcessorError> {
        info!("[{LOG_PATH}] bridging runes flow with request: {request:?}");
        crate::routes::bridge_runes_flow::handle(self, request).await?;
        Ok(BridgeRunesResponse {
            message: "message for btc address".to_string(),
        })
    }

    async fn run_spark_addr_issuing(
        &mut self,
        request: IssueSparkDepositAddressRequest,
    ) -> Result<IssueSparkDepositAddressResponse, FlowProcessorError> {
        info!("[{LOG_PATH}] issuing spark addr to user with request: {request:?}");
        let address = crate::routes::spark_addr_issuing::handle(self, request).await?;
        Ok(IssueSparkDepositAddressResponse {
            addr_to_replenish: address,
        })
    }

    #[tracing::instrument(level = "trace", skip(self, request), ret)]
    async fn run_exit_spark_flow(
        &mut self,
        request: ExitSparkRequest,
    ) -> Result<ExitSparkResponse, FlowProcessorError> {
        info!("[{LOG_PATH}] exiting spark flow with request: {request:?}");
        crate::routes::exit_spark_flow::handle(self, request).await?;
        Ok(ExitSparkResponse {
            message: "message for spark exit".to_string(),
        })
    }
}
