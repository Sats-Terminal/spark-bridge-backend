use async_trait::async_trait;
use frost::errors::AggregatorError;
use frost::traits::SignerClient as SignerClientTrait;
use frost::traits::*;
use frost::types::*;
use gateway_config_parser::config::VerifierConfig;
use reqwest::{Client, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub struct TxCheckerClient {
    config: VerifierConfig,
    client: Client,
}

const CHECK_TX_PATH: &str = "/api/gateway/check-tx";

impl TxCheckerClient {
    pub fn new(config: VerifierConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub async fn send_request<T: Serialize, U: DeserializeOwned>(
        &self,
        url: Url,
        request: T,
    ) -> Result<U, BtcAggregatorError> {
        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|e| BtcAggregatorError::Internal(format!("Failed to send HTTP request: {:?}", e)))?;

        if response.status().is_success() {
            let response: U = response
                .json()
                .await
                .map_err(|e| BtcAggregatorError::Internal(format!("Failed to deserialize response: {:?}", e)))?;
            Ok(response)
        } else {
            Err(BtcAggregatorError::HttpError(format!(
                "Failed to send HTTP request with status {}, error: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )))
        }
    }

    pub async fn get_url(&self, path: &str) -> Result<Url, BtcAggregatorError> {
        Url::parse(&format!("{}{}", self.config.address, path))
            .map_err(|e| BtcAggregatorError::Internal(format!("Failed to parse URL: {:?}", e)))
    }
}

#[async_trait]
impl TxCheckerClientTrait for TxCheckerClient {
    async fn check_tx(&self, req: CheckTxRequest) -> Result<CheckTxResponse, BtcAggregatorError> {
        let url = self.get_url(CHECK_TX_PATH).await?;
        self.send_request(url, req).await
    }
}
