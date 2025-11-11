use crate::flow_router::FlowProcessorRouter;
use crate::rune_metadata_client::RuneMetadataClient;
use crate::types::*;
use bitcoin::Network;
use btc_indexer_client::client_api::IndexerClient;
use frost::aggregator::FrostAggregator;
use gateway_config_parser::config::VerifierConfig;
use gateway_local_db_store::storage::LocalDbStorage;
use gateway_rune_transfer::bitcoin_client::BitcoinClient;
use gateway_spark_service::service::SparkService;
use global_utils::common_types::get_uuid;
use spark_client::client::SparkRpcClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing;
use tracing::instrument;
use uuid::Uuid;

// This is core struct that handles flows execution
// For each request it creates a thread that runs the flow
pub struct FlowProcessor {
    pub verifier_configs: Arc<Vec<VerifierConfig>>,
    pub tx_receiver: mpsc::Receiver<(FlowProcessorMessage, OneshotFlowProcessorSender)>,
    pub flow_receiver: mpsc::Receiver<Uuid>,
    pub flow_sender: mpsc::Sender<Uuid>,
    pub storage: Arc<LocalDbStorage>,
    pub flows: HashMap<Uuid, JoinHandle<()>>,
    pub cancellation_token: CancellationToken,
    pub cancellation_retries: u64,
    pub frost_aggregator: Arc<FrostAggregator>,
    pub spark_service: Arc<SparkService>,
    pub spark_client: Arc<SparkRpcClient>,
    pub bitcoin_client: Arc<BitcoinClient>,
    pub rune_metadata_client: Option<Arc<RuneMetadataClient>>,
    pub network: Network,
    pub bitcoin_indexer: IndexerClient,
}

pub struct FlowProcessorInitArgs {
    pub verifier_configs: Arc<Vec<VerifierConfig>>,
    pub tx_receiver: mpsc::Receiver<(FlowProcessorMessage, OneshotFlowProcessorSender)>,
    pub storage: Arc<LocalDbStorage>,
    pub cancellation_retries: u64,
    pub frost_aggregator: Arc<FrostAggregator>,
    pub network: Network,
    pub cancellation_token: CancellationToken,
    pub spark_service: Arc<SparkService>,
    pub spark_client: Arc<SparkRpcClient>,
    pub bitcoin_client: Arc<BitcoinClient>,
    pub bitcoin_indexer: IndexerClient,
    pub rune_metadata_client: Option<Arc<RuneMetadataClient>>,
}

impl FlowProcessor {
    pub fn new(flow_processor_config: FlowProcessorInitArgs) -> Self {
        let (flow_sender, flow_receiver) = mpsc::channel::<Uuid>(1000);
        Self {
            verifier_configs: flow_processor_config.verifier_configs,
            tx_receiver: flow_processor_config.tx_receiver,
            flow_receiver,
            flow_sender,
            storage: flow_processor_config.storage,
            flows: HashMap::default(),
            cancellation_token: flow_processor_config.cancellation_token,
            cancellation_retries: flow_processor_config.cancellation_retries,
            frost_aggregator: flow_processor_config.frost_aggregator,
            spark_service: flow_processor_config.spark_service,
            spark_client: flow_processor_config.spark_client,
            bitcoin_client: flow_processor_config.bitcoin_client,
            rune_metadata_client: flow_processor_config.rune_metadata_client,
            network: flow_processor_config.network,
            bitcoin_indexer: flow_processor_config.bitcoin_indexer,
        }
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                flow = self.flow_receiver.recv() => {
                    match flow {
                        None => {
                            tracing::error!("Task channel closed unexpectedly");
                            break;
                        }
                        Some(flow_id) => {
                            tracing::info!("Received task for id {}", flow_id);
                            let _ = self.flows.remove(&flow_id);
                        }
                    }
                }
                wrapper = self.tx_receiver.recv() => {
                    match wrapper {
                        None => {
                            tracing::error!("Message channel closed unexpectedly");
                            break;
                        }
                        Some(wrapper) => {
                            tracing::info!("Received message");

                            let (message, response_sender) = wrapper;

                            let flow_id = get_uuid();

                            let router = FlowProcessorRouter{
                                verifier_configs: self.verifier_configs.clone(),
                                storage: self.storage.clone(),
                                flow_id,
                                response_sender,
                                task_sender:  self.flow_sender.clone(),
                                frost_aggregator: self.frost_aggregator.clone(),
                                spark_service: self.spark_service.clone(),
                                spark_client: self.spark_client.clone(),
                                network: self.network,
                                bitcoin_client: self.bitcoin_client.clone(),
                                bitcoin_indexer: self.bitcoin_indexer.clone(),
                                rune_metadata_client: self.rune_metadata_client.clone(),
                            };

                            let handle = tokio::task::spawn(async move {
                                tracing::info!("Running flow for id {}", flow_id);
                                router.run(message).await;
                                tracing::info!("Flow for id {} finished", flow_id);
                            });

                            self.flows.insert(flow_id, handle);
                        }
                    }
                }
                _ = self.cancellation_token.cancelled() => {
                    tracing::info!("Shutting down flow processor");

                    for i in 0..self.cancellation_retries {
                        if self.flows.is_empty() {
                            return;
                        }
                        while let Ok(flow_id) = self.flow_receiver.try_recv() {
                            let _ = self.flows.remove(&flow_id);
                        }
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        tracing::info!("Waiting flows to finish {}/{}", i + 1, self.cancellation_retries);
                    }

                    for (flow_id, handle) in self.flows.iter() {
                        handle.abort();
                        tracing::info!("Aborted flow for id {}", flow_id);
                    }

                    self.flows.clear();
                    break;
                }
            }
        }
    }
}
