use crate::error::GatewayClientError;
use reqwest::Client;
use verifier_config_parser::config::GatewayConfig;
use bitcoin::Txid;
use verifier_local_db_store::schemas::deposit_address::DepositStatus;
use serde::{Serialize, Deserialize};

const NOTIFY_RUNES_DEPOSIT_PATH: &str = "/api/verifier/notify-runes-deposit";

#[derive(Clone, Debug)]
pub struct GatewayClient {
    config: GatewayConfig,
    client: Client,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotifyRunesDepositRequest {
    pub verifier_id: u16,
    pub txid: Txid,
    pub verifier_response: DepositStatus,
}

impl GatewayClient {
    pub fn new(config: GatewayConfig) -> Self {
        Self { config, client: Client::new() }
    }

    pub async fn notify_runes_deposit(&self, request: NotifyRunesDepositRequest) -> Result<(), GatewayClientError> {
        let url = self.config.address.join(NOTIFY_RUNES_DEPOSIT_PATH)
            .map_err(|e| GatewayClientError::DeserializeError(format!("Failed to join URL: {:?}", e)))?;

        let response = self.client.post(url).json(&request).send().await
            .map_err(|e| GatewayClientError::HttpError(format!("Failed to send request: {:?}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(GatewayClientError::HttpError(format!("Failed to send HTTP request with status {}, error: {}", response.status(), response.text().await.unwrap_or_default())))
        }
    }
}