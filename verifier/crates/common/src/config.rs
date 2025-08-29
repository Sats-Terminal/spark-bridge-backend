use serde::Deserialize;
use toml;

const CONFIG_PATH: &str = "verifier/config.toml";

#[derive(Debug, Deserialize)]
pub struct VerifierConfig {
    pub server: ServerConfig,
    #[serde(rename = "btc-indexer")]
    pub btc_indexer: BtcIndexerConfig,
    #[serde(rename = "spark-balance-checker")]
    pub spark_balance_checker: SparkBalanceCheckerConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct BtcIndexerConfig {
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct SparkBalanceCheckerConfig {
    pub address: String,
}

impl VerifierConfig {
    pub fn new(config_path: Option<&str>) -> Self {
        let path = config_path.unwrap_or(CONFIG_PATH);
        let config_file = std::fs::read_to_string(path).unwrap();
        let config: VerifierConfig = toml::from_str(&config_file).unwrap();
        config
    }
}
