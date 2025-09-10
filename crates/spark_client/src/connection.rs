use std::str::FromStr;

use spark_protos::spark::spark_service_client::SparkServiceClient;
use spark_protos::spark_authn::spark_authn_service_client::SparkAuthnServiceClient;
use spark_protos::spark_token::spark_token_service_client::SparkTokenServiceClient;
use tonic::transport::{Channel, ClientTlsConfig, Uri};

use crate::common::{config::SparkConfig, error::SparkClientError};

#[derive(Clone)]
pub struct SparkServicesClients {
    pub spark: SparkServiceClient<Channel>,
    pub spark_token: SparkTokenServiceClient<Channel>,
    pub spark_auth: SparkAuthnServiceClient<Channel>,
}

pub struct SparkTlsConnection {
    spark_config: SparkConfig,
    coordinator_operator: usize,
}

impl SparkTlsConnection {
    pub(crate) fn new(config: SparkConfig) -> Result<Self, SparkClientError> {
        let coordinator_operator = config.coordinator_operator()?;
        Ok(SparkTlsConnection {
            spark_config: config,
            coordinator_operator,
        })
    }

    async fn create_tls_channel(&self) -> Result<Channel, SparkClientError> {
        let base_url = self.spark_config.operators[self.coordinator_operator].base_url.clone();
        let uri = Uri::from_str(base_url.0.as_ref())
            .map_err(|e| SparkClientError::ConnectionError(format!("Failed to create URI: {}", e)))?;
        let mut tls = ClientTlsConfig::new().ca_certificate(self.spark_config.ca_pem.clone());
        if let Some(host) = uri.host() {
            tls = tls.domain_name(host);
        }

        let channel = Channel::from_shared(uri.to_string())
            .map_err(|e| SparkClientError::ConnectionError(format!("Failed to create channel: {}", e)))?
            .tls_config(tls)
            .map_err(|e| SparkClientError::ConnectionError(format!("Failed to create TLS config: {}", e)))?
            .connect()
            .await
            .map_err(|e| SparkClientError::ConnectionError(format!("Failed to connect to operator: {}", e)))?;

        Ok(channel)
    }

    // This function creates a new spark client.
    pub(crate) async fn create_clients(&self) -> Result<SparkServicesClients, SparkClientError> {
        let channel = self.create_tls_channel().await?;

        Ok(SparkServicesClients {
            spark: SparkServiceClient::new(channel.clone()),
            spark_token: SparkTokenServiceClient::new(channel.clone()),
            spark_auth: SparkAuthnServiceClient::new(channel),
        })
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
                id: 0,
                identity_public_key: "".to_string(),
                frost_identifier: "".to_string(),
                running_authority: "".to_string(),
                is_coordinator: Some(true),
            }],
            ca_pem: CaCertificate::from_path("../../spark_balance_checker/infrastructure/configuration/ca.pem")?.ca_pem,
        };
        let connection = SparkTlsConnection::new(spark_config).unwrap();
        connection.create_clients().await.unwrap();
        Ok(())
    }
}
