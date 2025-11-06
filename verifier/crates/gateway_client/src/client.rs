use crate::error::GatewayClientError;
use bitcoin::OutPoint;
use reqwest::Client;
use serde::Serialize;
use tracing;
use tracing::instrument;
use uuid::Uuid;
use verifier_config_parser::config::GatewayConfig;

const NOTIFY_RUNES_DEPOSIT_PATH: &str = "/api/verifier/notify-runes-deposit";
const NOTIFY_SPARK_DEPOSIT_PATH: &str = "/api/verifier/notify-spark-deposit";

#[derive(Clone, Debug)]
pub struct GatewayClient {
    config: GatewayConfig,
    client: Client,
}

#[derive(Serialize, Debug, Clone)]
pub enum GatewayDepositStatus {
    Confirmed,
    Failed,
}

#[derive(Serialize, Debug, Clone)]
pub struct GatewayNotifyRunesDepositRequest {
    pub verifier_id: u16,
    pub request_id: Uuid,
    pub outpoint: OutPoint,
    pub sats_amount: u64,
    pub status: GatewayDepositStatus,
    pub error_details: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct GatewayNotifySparkDepositRequest {
    pub verifier_id: u16,
    pub request_id: Uuid,
    pub spark_address: String,
    pub status: GatewayDepositStatus,
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

    #[instrument(level = "trace", skip(self), ret)]
    pub async fn notify_spark_deposit(
        &self,
        request: GatewayNotifySparkDepositRequest,
    ) -> Result<(), GatewayClientError> {
        let url = self
            .config
            .address
            .join(NOTIFY_SPARK_DEPOSIT_PATH)
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
}
