use spark_protos::spark_token::spark_token_service_client::SparkTokenServiceClient;
use tonic::transport::{Channel, ClientTlsConfig, Uri, Certificate};
use crate::error::SparkClientError;
use std::{io::Read, str::FromStr};
use spark_protos::spark_token::QueryTokenOutputsRequest;
use spark_address::{decode_spark_address};
use tracing;
use hex;
use token_identifier::TokenIdentifier;
use std::sync::Once;
use rustls;

fn install_rustls_provider() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("install rustls crypto provider");
    });
}

#[derive(Clone)]
pub struct SparkClient {
    client: SparkTokenServiceClient<Channel>,
    config: SparkClientConfig,
}

#[derive(Clone, Debug)]
pub struct SparkClientConfig {
    pub base_url: String,
    pub certificate_path: String,
}

#[derive(Clone, Debug)]
pub struct GetSparkAddressDataRequest {
    pub spark_address: String,
}

#[derive(Clone, Debug)]
pub struct GetSparkAddressDataResponse {
    pub token_identifier: TokenIdentifier,
    pub amount: u128,
}


impl SparkClient {
    pub async fn new(config: SparkClientConfig) -> Result<Self, SparkClientError> {
        install_rustls_provider();
        let channel = create_tls_channel(config.clone()).await?;
        Ok(Self { client: SparkTokenServiceClient::new(channel), config })
    }

    pub async fn get_spark_address_data(&mut self, request: GetSparkAddressDataRequest) -> Result<GetSparkAddressDataResponse, SparkClientError> {
        tracing::debug!("Getting spark address data for {}", request.spark_address);

        let address_data = decode_spark_address(&request.spark_address)?;
        let public_key = hex::decode(address_data.identity_public_key).unwrap();
        let response = self.client.query_token_outputs(
            QueryTokenOutputsRequest {
                owner_public_keys: vec![public_key],
                token_identifiers: vec![],
                issuer_public_keys: vec![],
                network: 2, // Regtest
                page_request: None,
            }
        ).await?.into_inner();

        assert_eq!(response.outputs_with_previous_transaction_data.len(), 1);
        let output = response.outputs_with_previous_transaction_data[0].output.as_ref().unwrap();

        let token_identifier = TokenIdentifier::from_bytes(output.token_identifier.as_ref().unwrap()).unwrap();
        let amount = u128::from_be_bytes(output.token_amount.clone().try_into().unwrap());

        tracing::debug!("Token identifier: {:?}", token_identifier);
        tracing::debug!("Amount: {:?}", amount);

        Ok(GetSparkAddressDataResponse {
            token_identifier,
            amount,
        })
    }
}

async fn create_tls_channel(config: SparkClientConfig) -> Result<Channel, SparkClientError> {
    let uri = Uri::from_str(config.base_url.as_ref())
        .map_err(|e| SparkClientError::CreateTlsChannelError(format!("Failed to create URI: {}", e)))?;
    let mut tls = ClientTlsConfig::new();
    let certificate = Certificate::from_pem(std::fs::read(config.certificate_path.clone()).unwrap());
    tls = tls.ca_certificate(certificate);
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