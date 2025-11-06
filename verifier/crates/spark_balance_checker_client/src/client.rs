use crate::error::SparkBalanceCheckerClientError;
use reqwest::Client;
use serde::{Serialize, de::DeserializeOwned};
use spark_balance_checker_server::{
    init::{HEALTHCHECK_ENDPOINT, VERIFY_BALANCE_ENDPOINT, VERIFY_TRANSACTION_ENDPOINT},
    models::{DepositStatus, VerifyBalanceRequest, VerifyBalanceResponse, VerifyTransferRequest},
};
use tracing::instrument;
use url::Url;
pub use verifier_config_parser::config::SparkBalanceCheckerConfig;
use verifier_local_db_store::schemas::deposit_address::DepositStatus as DepositStatusDB;

#[derive(Clone, Debug)]
pub struct SparkBalanceCheckerClient {
    config: SparkBalanceCheckerConfig,
    client: Client,
}

// TODO: refactor code to have some common types shared between the system,
// avoid redefining the same types
pub fn cast_deposit_status(status: &DepositStatus) -> DepositStatusDB {
    match status {
        DepositStatus::Confirmed => DepositStatusDB::Confirmed,
        DepositStatus::Failed => DepositStatusDB::Failed,
    }
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
        let response =
            self.client.post(url).json(&request).send().await.map_err(|e| {
                SparkBalanceCheckerClientError::HttpError(format!("Failed to send HTTP request: {:?}", e))
            })?;

        if response.status().is_success() {
            let response: U = response.json().await.map_err(|e| {
                SparkBalanceCheckerClientError::DeserializeError(format!("Failed to deserialize response: {:?}", e))
            })?;
            Ok(response)
        } else {
            let status = response.status();
            let error_message = response.text().await.unwrap_or_default();
            tracing::error!(
                "Failed to send HTTP request with status {}, error: {}",
                status,
                error_message
            );
            Err(SparkBalanceCheckerClientError::HttpError(format!(
                "Failed to send HTTP request with status {}, error: {}",
                status, error_message
            )))
        }
    }

    async fn get_url(&self, path: &str) -> Result<Url, SparkBalanceCheckerClientError> {
        self.config
            .address
            .join(path)
            .map_err(|e| SparkBalanceCheckerClientError::DeserializeError(format!("Failed to join URL: {:?}", e)))
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub async fn verify_balance(
        &self,
        request: VerifyBalanceRequest,
    ) -> Result<VerifyBalanceResponse, SparkBalanceCheckerClientError> {
        let url = self.get_url(VERIFY_BALANCE_ENDPOINT).await?;

        Ok(self.send_request(url, request).await?)
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub async fn verify_transfer(
        &self,
        request: VerifyTransferRequest,
    ) -> Result<VerifyBalanceResponse, SparkBalanceCheckerClientError> {
        let url = self.get_url(VERIFY_TRANSACTION_ENDPOINT).await?;

        Ok(self.send_request(url, request).await?)
    }

    #[tracing::instrument(skip_all, err)]
    pub async fn healthcheck(&self) -> Result<(), SparkBalanceCheckerClientError> {
        let url = self.get_url(HEALTHCHECK_ENDPOINT).await?;

        self.send_request(url, ()).await
    }
}
