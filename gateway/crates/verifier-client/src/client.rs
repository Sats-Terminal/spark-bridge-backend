use crate::error::VerifierClientError;
use gateway_config_parser::config::VerifierConfig;
use reqwest::{Client, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Clone, Debug)]
pub struct VerifierClient {
    config: VerifierConfig,
    client: Client,
}

impl VerifierClient {
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
            tracing::error!("Failed to send HTTP request with status {}", response.status());
            Err(VerifierClientError::HttpError(format!(
                "Failed to send HTTP request with status {}, error: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )))
        }
    }

    pub async fn get_url(&self, path: &str) -> Result<Url, VerifierClientError> {
        Url::parse(&format!("{}{}", self.config.address, path))
            .map_err(|e| VerifierClientError::DeserializeError(format!("Failed to parse URL: {:?}", e)))
    }
}
