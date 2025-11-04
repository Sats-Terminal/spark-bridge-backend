use bitcoin::Network;
use config::Config;
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
}

// FIXME!!! This is super bad hardcode, delete these as soon as we have proper indexer logic
pub fn construct_hardcoded_callback_url(app_config: &AppConfig) -> Url {
    Url::parse(&format!(
        "http://{}:{}/api/btc-indexer/notify-runes-deposit",
        app_config.ip, app_config.port
    ))
    .unwrap()
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
}

impl ServerConfig {
    pub fn init_config(path: String) -> Self {
        let config = Config::builder()
            .add_source(config::File::with_name(&path))
            .build()
            .unwrap();
        config.try_deserialize().unwrap()
    }
}
