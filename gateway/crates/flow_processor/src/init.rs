use crate::flow_processor::FlowProcessor;
use crate::flow_sender::FlowSender;
use bitcoin::Network;
use frost::aggregator::FrostAggregator;
use gateway_local_db_store::storage::LocalDbStorage;
use gateway_config_parser::config::ServerConfig;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use gateway_spark_service::service::SparkService;
use spark_client::client::SparkRpcClient;
use bitcoin::secp256k1::PublicKey;
use std::str::FromStr;

pub async fn create_flow_processor(
    server_config: ServerConfig,
    storage: Arc<LocalDbStorage>,
    cancellation_retries: u64,
    frost_aggregator: FrostAggregator,
    network: Network,
) -> (FlowProcessor, FlowSender) {
    let (tx_sender, tx_receiver) = mpsc::channel(1000);
    let cancellation_token = CancellationToken::new();
    let spark_client = SparkRpcClient::new(server_config.spark.clone()).await.unwrap();
    let spark_operator_identity_public_keys = server_config.spark.operators.iter()
        .map(|o| PublicKey::from_str(&o.identity_public_key).unwrap()).collect();
    let spark_service = SparkService::new(
        spark_client.clone(),
        frost_aggregator.clone(),
        spark_operator_identity_public_keys,
    );
    let flow_processor = FlowProcessor::new(
        Arc::new(server_config.verifiers.0),
        tx_receiver,
        storage,
        cancellation_retries,
        frost_aggregator,
        network,
        cancellation_token.clone(),
        Arc::new(spark_service),
        Arc::new(spark_client),
    );
    let flow_sender = FlowSender::new(tx_sender, cancellation_token);
    (flow_processor, flow_sender)
}
