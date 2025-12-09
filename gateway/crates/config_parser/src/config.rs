use bitcoin::secp256k1::PublicKey;
use btc_indexer_config::IndexerClientConfig;
use config::{Config, Environment};
use global_utils::network::NetworkConfig;
use serde::{Deserialize, Serialize};
use spark_client::common::config::SparkConfig;
use std::{collections::BTreeMap, env};

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
    pub public_key: PublicKey,
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
        let verifier_env_overrides = capture_verifier_address_overrides();

        let config = Config::builder()
            .add_source(config::File::with_name(&path))
            .add_source(
                Environment::with_prefix("GATEWAY")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .add_source(
                Environment::with_prefix("GATEWAY")
                    .prefix_separator("__")
                    .separator("__"),
            )
            .build()
            .unwrap();

        let mut server_config: Self = config.try_deserialize().unwrap();
        apply_verifier_address_overrides(&mut server_config.verifiers, verifier_env_overrides);
        server_config
    }
}

fn capture_verifier_address_overrides() -> BTreeMap<usize, String> {
    const PREFIX: &str = "GATEWAY__VERIFIERS_CONFIG__";
    const SUFFIX: &str = "__ADDRESS";
    let mut overrides = BTreeMap::new();

    // Collect first so we can safely mutate the environment while iterating.
    for (key, value) in env::vars().collect::<Vec<_>>() {
        if let Some(rest) = key.strip_prefix(PREFIX) {
            if let Some(index_part) = rest.strip_suffix(SUFFIX) {
                if let Ok(index) = index_part.parse::<usize>() {
                    overrides.insert(index, value);
                    unsafe {
                        env::remove_var(key);
                    }
                }
            }
        }
    }

    overrides
}

fn apply_verifier_address_overrides(verifiers: &mut VerifiersConfig, overrides: BTreeMap<usize, String>) {
    for (index, address) in overrides {
        if let Some(verifier) = verifiers.0.get_mut(index) {
            verifier.address = address;
        }
    }
}
