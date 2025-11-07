use btc_indexer_config::IndexerClientConfig;
use config::{Config, Environment};
use global_utils::network::NetworkConfig;
use serde::{Deserialize, Serialize};
use spark_client::common::config::SparkConfig;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct AppConfig {
    #[serde(rename = "http_server_ip")]
    pub ip: String,
    #[serde(rename = "http_server_port")]
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct VerifierConfig {
    pub id: u16,
    pub address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub struct AggregatorConfig {
    pub threshold: u16,
    pub total_participants: u16,
    /// Need for configuring Btc confirmation aggregator, can be used as an additional argument in Frost Aggreagator
    pub update_interval_milliseconds: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub struct FlowProcessorConfig {
    pub cancellation_retries: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(transparent)]
pub struct VerifiersConfig(pub Vec<VerifierConfig>);

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct BitcoinClientConfig {
    pub url: String,
    #[serde(flatten)]
    pub auth: Option<BitcoinNodeAuth>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct BitcoinNodeAuth {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub struct DkgPregenConfig {
    pub update_interval_millis: u64,
    pub min_threshold: u64,
    pub max_cached: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct FeeConfig {
    pub amount: u64,
    pub btc_address: String,
    pub spark_address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(rename(deserialize = "application"))]
    pub server_public: AppConfig,
    #[serde(rename = "verifiers_config")]
    pub verifiers: VerifiersConfig,
    #[serde(rename = "aggregator_config")]
    pub aggregator: AggregatorConfig,
    #[serde(rename = "dkg_pregen_config")]
    pub dkg_pregen_config: DkgPregenConfig,
    #[serde(rename = "flow_processor_config")]
    pub flow_processor: FlowProcessorConfig,
    #[serde(rename = "network_config")]
    pub network: NetworkConfig,
    #[serde(rename = "database_config")]
    pub database: DatabaseConfig,
    #[serde(rename = "spark_config")]
    pub spark: SparkConfig,
    #[serde(rename = "bitcoin_client_config")]
    pub bitcoin_client: BitcoinClientConfig,
    #[serde(rename = "bitcoin_indexer_client_config")]
    pub bitcoin_indexer_client: IndexerClientConfig,
    #[serde(rename = "fee_config")]
    pub fee: Option<FeeConfig>,
}

impl ServerConfig {
    pub fn init_config(path: String) -> Self {
        let config = Config::builder()
            .add_source(config::File::with_name(&path))
            .add_source(
                Environment::with_prefix("GATEWAY")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()
            .unwrap();
        config.try_deserialize().unwrap()
    }
}
