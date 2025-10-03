use crate::error::GatewayClientError;
use bitcoin::OutPoint;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing;
use tracing::instrument;
use verifier_config_parser::config::GatewayConfig;
use verifier_local_db_store::schemas::deposit_address::DepositStatus;

const NOTIFY_RUNES_DEPOSIT_PATH: &str = "/api/verifier/notify-runes-deposit";
const HEALTHCHECK_PATH: &str = "/health";

#[derive(Clone, Debug)]
pub struct GatewayClient {
    config: GatewayConfig,
    client: Client,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GatewayNotifyRunesDepositRequest {
    pub verifier_id: u16,
    pub out_point: OutPoint,
    pub sats_fee_amount: u64,
    pub status: DepositStatus,
}

impl GatewayClient {
    pub fn new(config: GatewayConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub async fn notify_runes_deposit(
        &self,
        request: GatewayNotifyRunesDepositRequest,
    ) -> Result<(), GatewayClientError> {
        let url = self
            .config
            .address
            .join(NOTIFY_RUNES_DEPOSIT_PATH)
            .map_err(|e| GatewayClientError::DeserializeError(format!("Failed to join URL: {:?}", e)))?;

        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|e| GatewayClientError::HttpError(format!("Failed to send request: {:?}", e)))?;

        if response.status().is_success() {
            tracing::info!(
                "Request to notify runes deposit for verifier: {} successful",
                request.verifier_id
            );
            Ok(())
        } else {
            tracing::error!(
                "Failed to send HTTP request for {:?}, with status {}",
                request,
                response.status()
            );
            Err(GatewayClientError::HttpError(format!(
                "Failed to send HTTP request with status {}, error: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )))
        }
    }

    #[tracing::instrument(skip_all, err)]
    pub async fn healthcheck(&self) -> Result<(), GatewayClientError> {
        tracing::info!("Sending healthcheck to gateway for verifier",);
        let url = self
            .config
            .address
            .join(HEALTHCHECK_PATH)
            .map_err(|e| GatewayClientError::DeserializeError(format!("Failed to join URL: {:?}", e)))?;

        let response = self
            .client
            .post(url)
            .send()
            .await
            .map_err(|e| GatewayClientError::HttpError(format!("Failed to send request: {:?}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            tracing::error!("Failed to send HTTP request with status {}", response.status());
            Err(GatewayClientError::HttpError(format!(
                "Failed to send {HEALTHCHECK_PATH} HTTP request with status {}, error: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )))
        }
    }
}
