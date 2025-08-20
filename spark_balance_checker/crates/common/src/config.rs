use serde::Deserialize;
use spark_client::SparkConfig;
use toml;

const CONFIG_FILE: &str = "./spark_balance_checker/config.toml";

#[derive(Deserialize, Debug, Clone)]
pub struct ServerConfig {
    pub address: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub spark: SparkConfig,
}

impl Config {
    pub fn new(file_path: Option<&str>) -> Self {
        let file_path = file_path.unwrap_or(CONFIG_FILE);
        let file_content = std::fs::read_to_string(file_path).unwrap();
        let config: Config = toml::from_str(&file_content).unwrap();
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        Config::new(None);
    }
}
