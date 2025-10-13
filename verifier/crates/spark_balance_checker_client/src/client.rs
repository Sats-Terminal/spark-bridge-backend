use crate::error::SparkBalanceCheckerClientError;
use reqwest::Client;
use serde::{Serialize, de::DeserializeOwned};
use serde::Deserialize;
use token_identifier::TokenIdentifier;
use tracing::instrument;
pub use verifier_config_parser::config::SparkBalanceCheckerConfig;
use url::Url;
use uuid::Uuid;

const GET_BALANCE_PATH: &str = "/verify-balance";
const HEALTHCHECK_PATH: &str = "/health";

#[derive(Clone, Debug)]
pub struct SparkBalanceCheckerClient {
    config: SparkBalanceCheckerConfig,
    client: Client,
}

#[derive(Serialize, Debug)]
pub struct VerifyBalanceRequest {
    pub request_id: Uuid,
    pub spark_address: String,
    pub token_identifier: TokenIdentifier,
    pub amount: u128,
}

#[derive(Deserialize, Debug)]
pub enum VerificationStatus {
    Confirmed,
    Failed,
}

#[derive(Deserialize, Debug)]
pub struct VerifyBalanceResponse {
    pub verification_status: VerificationStatus,
    pub error_details: Option<String>,
}

impl SparkBalanceCheckerClient {
    pub fn new(config: SparkBalanceCheckerConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    async fn send_request<T: Serialize, U: DeserializeOwned>(
        &self,
        url: Url,
        request: T,
    ) -> Result<U, SparkBalanceCheckerClientError> {
        let response = self.client.post(url).json(&request).send().await
            .map_err(|e| SparkBalanceCheckerClientError::HttpError(format!("Failed to send HTTP request: {:?}", e)))?;

        if response.status().is_success() {
            let response: U = response.json().await.map_err(|e| SparkBalanceCheckerClientError::DeserializeError(format!("Failed to deserialize response: {:?}", e)))?;
            Ok(response)
        } else {
            let status = response.status();
            let error_message = response.text().await.unwrap_or_default();
            tracing::error!("Failed to send HTTP request with status {}, error: {}", status, error_message);
            Err(SparkBalanceCheckerClientError::HttpError(format!("Failed to send HTTP request with status {}, error: {}", status, error_message)))
        }
    }

    async fn get_url(&self, path: &str) -> Result<Url, SparkBalanceCheckerClientError> {
        self.config.address.join(path).map_err(|e| {
            SparkBalanceCheckerClientError::DeserializeError(format!("Failed to join URL: {:?}", e))
        })
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub async fn verify_balance(
        &self,
        request: VerifyBalanceRequest,
    ) -> Result<VerifyBalanceResponse, SparkBalanceCheckerClientError> {
        let url = self.get_url(GET_BALANCE_PATH).await?;
        
        self.send_request(url, request).await
    }

    #[tracing::instrument(skip_all, err)]
    pub async fn healthcheck(&self) -> Result<(), SparkBalanceCheckerClientError> {
        let url = self.get_url(HEALTHCHECK_PATH).await?;
        
        self.send_request(url, ()).await
    }
}
