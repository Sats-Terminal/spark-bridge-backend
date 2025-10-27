use config::Config;
use serde::{Deserialize, Serialize};
use bitcoin::Network;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub btc_indexer: BtcIndexerConfig,
    pub database: DatabaseConfig,
    pub network: NetworkConfig,
    pub indexer_client: IndexerClientConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    pub hostname: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkConfig {
    pub network: Network,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BtcIndexerConfig {
    pub indexer_update_interval_millis: u64,
    pub validation_timeout_millis: u64,
    pub confirmation_block_height_delta: u64,
    pub tx_indexer_update_interval_millis: u64,
    pub start_block_height: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexerClientConfig {
    pub url: String,
}

impl AppConfig {
    pub fn init_config(path: String) -> Self {
        let config = Config::builder()
            .add_source(config::File::with_name(&path))
            .build()
            .unwrap();
        config.try_deserialize().unwrap()
    }
}
