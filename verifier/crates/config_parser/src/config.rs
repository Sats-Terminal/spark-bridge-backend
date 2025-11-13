use bitcoin::{Network, secp256k1::SecretKey};
use config::{Config, Environment};
use global_utils::common_types::Url;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct AppConfig {
    #[serde(rename = "http_server_ip")]
    pub ip: String,
    #[serde(rename = "http_server_port")]
    pub port: u16,
    #[serde(rename = "network")]
    pub network: Network,
    pub callback_url: Url,
    pub secret_key: SecretKey,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct BtcIndexerConfig {
    pub address: Url,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SparkBalanceCheckerConfig {
    pub address: Url,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SignerConfig {
    pub identifier: u16,
    pub threshold: u16,
    pub total_participants: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct GatewayConfig {
    pub address: Url,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct DatabaseConfig {
    pub url: String,
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
    pub server: AppConfig,
    #[serde(rename = "btc_indexer_url_config")]
    pub btc_indexer: BtcIndexerConfig,
    #[serde(rename = "spark_balance_checker_url_config")]
    pub spark_balance_checker: SparkBalanceCheckerConfig,
    #[serde(rename = "frost_signer_config")]
    pub frost_signer: SignerConfig,
    #[serde(rename = "gateway_config")]
    pub gateway: GatewayConfig,
    #[serde(rename = "database_config")]
    pub database: DatabaseConfig,
    #[serde(rename = "fee_config")]
    pub fee: Option<FeeConfig>,
}

impl ServerConfig {
    pub fn init_config(path: String) -> Self {
        let config = Config::builder()
            .add_source(config::File::with_name(&path))
            .add_source(
                Environment::with_prefix("VERIFIER")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()
            .unwrap();
        config.try_deserialize().unwrap()
    }
}
