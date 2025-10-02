use crate::error::VerifierClientError;
use gateway_config_parser::config::VerifierConfig;
use gateway_deposit_verification::error::DepositVerificationError;
use reqwest::{Client, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::instrument;

#[derive(Clone, Debug)]
pub struct VerifierClient {
    config: VerifierConfig,
    client: Client,
}

impl VerifierClient {
    const HEALTHCHECK_PATH: &'static str = "/health";

    pub fn new(config: VerifierConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub async fn send_post_json_request<T: Serialize, U: DeserializeOwned>(
        &self,
        url: Url,
        request: T,
    ) -> Result<U, VerifierClientError> {
        tracing::info!("Sending request to URL: {}", url);
        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|e| VerifierClientError::DeserializeError(format!("Failed to send HTTP request: {:?}", e)))?;

        if response.status().is_success() {
            let response: U = response.json().await.map_err(|e| {
                VerifierClientError::DeserializeError(format!("Failed to deserialize response: {:?}", e))
            })?;
            Ok(response)
        } else {
            Err(VerifierClientError::HttpError(format!(
                "Failed to send HTTP request with status {}, error: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )))
        }
    }

    pub async fn send_empty_post_request(&self, url: Url) -> Result<(), VerifierClientError> {
        tracing::info!("Sending request to URL: {}", url);
        let response = self
            .client
            .post(url.clone())
            .send()
            .await
            .map_err(|e| VerifierClientError::DeserializeError(format!("Failed to send HTTP request: {:?}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(VerifierClientError::HttpError(format!(
                "Failed to send '{url}' HTTP request with status {}, error: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )))
        }
    }

    pub async fn get_url(&self, path: &str) -> Result<Url, VerifierClientError> {
        Url::parse(&format!("{}{}", self.config.address, path))
            .map_err(|e| VerifierClientError::DeserializeError(format!("Failed to parse URL: {:?}", e)))
    }

    #[instrument(skip(self), err)]
    pub async fn healthcheck(&self) -> Result<(), DepositVerificationError> {
        let url = self.get_url(Self::HEALTHCHECK_PATH).await.map_err(|e| {
            DepositVerificationError::InvalidRequest(format!(
                "Failed to get URL for checking status of verifier: {}",
                e
            ))
        })?;

        self.send_empty_post_request(url).await.map_err(|e| {
            DepositVerificationError::HttpError(format!(
                "Failed to send request for checking health of verifier: {}",
                e
            ))
        })
    }
}
