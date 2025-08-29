use serde::Deserialize;

const DEFAULT_CONFIG_PATH: &str = "gateway/config.toml";

#[derive(Deserialize, Debug)]
pub struct GatewayConfig {
    pub server: ServerConfig,
    pub verifiers: Vec<VerifierConfig>,
}

#[derive(Deserialize, Debug)]
pub struct ServerConfig {
    pub address: String,
}

#[derive(Deserialize, Debug)]
pub struct VerifierConfig {
    pub id: u64,
    pub address: String,
}

impl GatewayConfig {
    pub fn new(config_path: Option<&str>) -> Self {
        let config_path = config_path.unwrap_or(DEFAULT_CONFIG_PATH);
        let config_str = std::fs::read_to_string(config_path).unwrap();
        let config: GatewayConfig = toml::from_str(&config_str).unwrap();
        config
    }
}