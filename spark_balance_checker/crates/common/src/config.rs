use eyre::Result;
use serde::Deserialize;
use toml;

const CONFIG_FILE: &str = "./spark_balance_checker/config.toml";

#[derive(Deserialize, Debug, Clone)]
pub struct SparkOperatorConfig {
    pub base_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SparkConfig {
    pub operators: Vec<SparkOperatorConfig>,
    pub ca_pem_path: String,
}

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
    pub fn new(file_path: Option<&str>) -> Result<Self> {
        let file_path = file_path.unwrap_or(CONFIG_FILE);
        let file_content = std::fs::read_to_string(file_path)?;
        let config: Config = toml::from_str(&file_content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        Config::new(None).unwrap();
    }
}
