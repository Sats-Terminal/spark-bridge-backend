use std::str::FromStr;

use spark_protos::spark::spark_service_client::SparkServiceClient;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Uri};

use crate::SparkOperatorConfig;
use crate::common::{config::SparkConfig, error::SparkClientError};

pub(crate) struct SparkConnectionPool {
    current_connection: usize,
    certificate: Certificate,
    operators: Vec<SparkOperatorConfig>,
}

impl SparkConnectionPool {
    pub(crate) fn new(config: SparkConfig) -> Self {
        SparkConnectionPool {
            current_connection: 0,
            certificate: config.ca_pem,
            operators: config.operators,
        }
    }

    async fn create_tls_channel(&self, base_url: String) -> Result<Channel, SparkClientError> {
        let uri = Uri::from_str(&base_url)
            .map_err(|e| SparkClientError::ConnectionError(format!("Failed to create URI: {}", e)))?;
        let mut tls = ClientTlsConfig::new().ca_certificate(self.certificate.clone());
        if let Some(host) = uri.host() {
            tls = tls.domain_name(host);
        }

        let channel = Channel::from_shared(uri.to_string())
            .map_err(|e| SparkClientError::ConnectionError(format!("Failed to create channel: {}", e)))?
            .tls_config(tls)
            .map_err(|e| SparkClientError::ConnectionError(format!("Failed to create TLS config: {}", e)))?
            .connect()
            .await
            .map_err(|e| {
                SparkClientError::ConnectionError(format!(
                    "Failed to connect to operator {}: {}",
                    self.current_connection, e
                ))
            })?;

        Ok(channel)
    }

    // This function creates a new spark client.
    pub(crate) async fn create_client(&mut self) -> Result<SparkServiceClient<Channel>, SparkClientError> {
        let base_url = self.operators[self.current_connection].base_url.clone();

        let channel = self.create_tls_channel(base_url.0.to_string()).await?;

        Ok(SparkServiceClient::new(channel))
    }

    // This function switches to the next operator in the pool.
    pub(crate) async fn switch_operator(&mut self) {
        self.current_connection = (self.current_connection + 1) % self.operators.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::config::{CaCertificate, SparkOperatorConfig};
    use env_logger;
    use global_utils::common_types::{Url, UrlWrapped};
    use tokio;
    fn init_logger() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .is_test(true)
            .try_init();
    }

    #[tokio::test]
    async fn test_get_client() -> anyhow::Result<()> {
        init_logger();

        let spark_config = SparkConfig {
            operators: vec![SparkOperatorConfig {
                base_url: UrlWrapped(Url::from_str("https://0.spark.lightspark.com")?),
            }],
            ca_pem: CaCertificate::from_path("../../spark_balance_checker/infrastructure/configuration/ca.pem")?.ca_pem,
        };
        let mut connection_pool = SparkConnectionPool::new(spark_config);
        connection_pool.create_client().await?;
        Ok(())
    }
}
