use crate::error::FlowProcessorError;
use crate::flow_processor::{FlowProcessor, FlowProcessorInitArgs};
use crate::flow_sender::FlowSender;
use bitcoin::Network;
use bitcoin::secp256k1::PublicKey;
use btc_indexer_client::client_api::new_btc_indexer_client;
use frost::aggregator::FrostAggregator;
use gateway_config_parser::config::ServerConfig;
use gateway_local_db_store::storage::LocalDbStorage;
use gateway_rune_transfer::bitcoin_client::new_bitcoin_client;
use gateway_spark_service::service::SparkService;
use spark_client::client::SparkRpcClient;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub async fn create_flow_processor(
    server_config: ServerConfig,
    storage: Arc<LocalDbStorage>,
    cancellation_retries: u64,
    frost_aggregator: Arc<FrostAggregator>,
    network: Network,
) -> Result<(FlowProcessor, FlowSender), FlowProcessorError> {
    let (tx_sender, tx_receiver) = mpsc::channel(1000);

    let cancellation_token = CancellationToken::new();

    let spark_client = SparkRpcClient::new(server_config.spark.clone())
        .await
        .map_err(|e| FlowProcessorError::InitializationError(format!("Failed to create spark client: {}", e)))?;

    let spark_operator_identity_public_keys = server_config
        .spark
        .operators
        .iter()
        .map(|o| {
            PublicKey::from_str(&o.identity_public_key)
                .map_err(|e| FlowProcessorError::InitializationError(format!("Failed to parse public key: {}", e)))
        })
        .collect::<Result<Vec<PublicKey>, FlowProcessorError>>()?;

    let spark_service = SparkService::new(
        spark_client.clone(),
        frost_aggregator.clone(),
        spark_operator_identity_public_keys,
    );

    let bitcoin_indexer = new_btc_indexer_client(server_config.bitcoin_indexer_client);
    let bitcoin_client = new_bitcoin_client(server_config.bitcoin_client.clone())
        .map_err(|e| FlowProcessorError::InitializationError(format!("Failed to create bitcoin client: {}", e)))?;

    let flow_processor = FlowProcessor::new(FlowProcessorInitArgs {
        verifier_configs: Arc::new(server_config.verifiers.0),
        tx_receiver,
        storage,
        cancellation_retries,
        frost_aggregator,
        network,
        cancellation_token: cancellation_token.clone(),
        spark_service: Arc::new(spark_service),
        spark_client: Arc::new(spark_client),
        bitcoin_client: Arc::new(bitcoin_client),
        bitcoin_indexer: bitcoin_indexer.clone(),
    });

    let flow_sender = FlowSender::new(bitcoin_indexer, tx_sender, cancellation_token);
    Ok((flow_processor, flow_sender))
}
