use crate::error::VerifierClientError;
use bitcoin::{
    hashes::{Hash, HashEngine, sha256::Hash as SHA256},
    key::Secp256k1,
    secp256k1::{Message, ecdsa::Signature},
};
use gateway_config_parser::config::VerifierConfig;
use hex;
use reqwest::{Client, Url, header::HeaderMap};
use serde::{Serialize, de::DeserializeOwned};
use tracing::error;

#[derive(Clone, Debug)]
pub struct VerifierClient {
    config: VerifierConfig,
    pub(crate) client: Client,
}

impl VerifierClient {
    pub fn new(config: VerifierConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub fn get_id(&self) -> u16 {
        self.config.id
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
            let headers = response.headers().clone();
            let body_text = response.text().await?;
            self.verify_signature(&headers, &body_text)?;

            Ok(serde_json::from_str::<U>(&body_text).map_err(|e| {
                VerifierClientError::DeserializeError(format!("Failed to deserialize response: {:?}", e))
            })?)
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

    fn verify_signature(&self, headers: &HeaderMap, body_text: &str) -> Result<(), VerifierClientError> {
        if let Some(key) = &self.config.public_key {
            let sig_hex = headers
                .get("x-signature")
                .ok_or_else(|| VerifierClientError::InvalidRequest("Missing 'x-signature' header".to_string()))?
                .to_str()
                .map_err(|_| VerifierClientError::InvalidRequest("Invalid 'x-signature' header format".to_string()))?;

            let sig_bytes = hex::decode(sig_hex)?;
            let signature = Signature::from_der(&sig_bytes)?;

            let mut hasher = SHA256::engine();
            hasher.input(body_text.as_bytes());
            let hash = SHA256::from_engine(hasher);
            let msg = Message::from_digest(hash.to_byte_array());

            let secp = Secp256k1::new();
            secp.verify_ecdsa(&msg, &signature, key).map_err(|err| {
                error!(
                    id = self.config.id,
                    address = self.config.address,
                    public_key = key.to_string(),
                    "Failed to verify signature: {:?}",
                    err
                );
                VerifierClientError::VerificationError(format!("Failed to verify signature: {:?}", err))
            })?;
        }

        Ok(())
    }
}
