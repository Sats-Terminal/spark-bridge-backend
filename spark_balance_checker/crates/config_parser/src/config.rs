use bitcoin::Network;
use config::{Config, Environment};
use serde::{Deserialize, Serialize};
use spark_client::common::config::SparkConfig;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(rename(deserialize = "application"))]
    pub app_config: AppConfig,
    #[serde(rename(deserialize = "network"))]
    pub network: NetworkConfig,
    #[serde(rename(deserialize = "spark_config"))]
    pub spark: SparkConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkConfig {
    pub network: Network,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(rename(deserialize = "http_server_ip"))]
    pub ip: String,
    #[serde(rename(deserialize = "http_server_port"))]
    pub port: u16,
}

impl ServerConfig {
    pub fn init_config(path: String) -> Self {
        let config = Config::builder()
            .add_source(config::File::with_name(&path))
            .add_source(
                Environment::with_prefix("SPARK_BALANCE_CHECKER")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()
            .unwrap();

        config.try_deserialize().unwrap()
    }
}
