use crate::error::SparkBalanceCheckerClientError;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use verifier_config_parser::config::SparkBalanceCheckerConfig;

const GET_BALANCE_PATH: &str = "/balance";
const HEALTHCHECK_PATH: &str = "/healthcheck";

#[derive(Clone, Debug)]
pub struct SparkBalanceCheckerClient {
    config: SparkBalanceCheckerConfig,
    client: Client,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetBalanceRequest {
    pub spark_address: String,
    pub rune_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetBalanceResponse {
    pub balance: u128,
}

impl SparkBalanceCheckerClient {
    pub fn new(config: SparkBalanceCheckerConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub async fn get_balance(
        &self,
        request: GetBalanceRequest,
    ) -> Result<GetBalanceResponse, SparkBalanceCheckerClientError> {
        let url =
            self.config.address.join(GET_BALANCE_PATH).map_err(|e| {
                SparkBalanceCheckerClientError::DeserializeError(format!("Failed to join URL: {:?}", e))
            })?;
        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|e| SparkBalanceCheckerClientError::HttpError(format!("Failed to send request: {:?}", e)))?;

        if response.status().is_success() {
            let response: GetBalanceResponse = response.json().await.map_err(|e| {
                SparkBalanceCheckerClientError::DeserializeError(format!("Failed to deserialize response: {:?}", e))
            })?;
            Ok(response)
        } else {
            Err(SparkBalanceCheckerClientError::HttpError(format!(
                "Failed to send HTTP request with status {}, error: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )))
        }
    }

    #[tracing::instrument(skip_all, err)]
    pub async fn healthcheck(&self) -> Result<(), SparkBalanceCheckerClientError> {
        let url =
            self.config.address.join(HEALTHCHECK_PATH).map_err(|e| {
                SparkBalanceCheckerClientError::DeserializeError(format!("Failed to join URL: {:?}", e))
            })?;
        let response = self
            .client
            .post(url)
            .send()
            .await
            .map_err(|e| SparkBalanceCheckerClientError::HttpError(format!("Failed to send request: {:?}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(SparkBalanceCheckerClientError::HttpError(format!(
                "Failed to send {HEALTHCHECK_PATH} HTTP request with status {}, error: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )))
        }
    }
}
