use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct SparkOperatorConfig {
    pub base_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SparkConfig {
    pub operators: Vec<SparkOperatorConfig>,
    pub ca_pem_path: String,
}
