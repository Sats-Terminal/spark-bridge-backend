use crate::client::VerifierClient;
use async_trait::async_trait;
use gateway_deposit_verification::error::DepositVerificationError;
use gateway_deposit_verification::traits::VerificationClient;
use gateway_deposit_verification::types::{
    WatchRunesDepositRequest, WatchRunesDepositResponse, WatchSparkDepositRequest, WatchSparkDepositResponse,
};

const WATCH_RUNES_DEPOSIT_PATH: &str = "/api/gateway/watch-runes-deposit";
const WATCH_SPARK_DEPOSIT_PATH: &str = "/api/gateway/watch-spark-deposit";

#[async_trait]
impl VerificationClient for VerifierClient {
    async fn watch_runes_deposit(
        &self,
        request: WatchRunesDepositRequest,
    ) -> Result<WatchRunesDepositResponse, DepositVerificationError> {
        let url = self.get_url(WATCH_RUNES_DEPOSIT_PATH).await.map_err(|e| {
            DepositVerificationError::InvalidRequest(format!("Failed to get URL for watch runes deposit: {}", e))
        })?;

        self.send_post_json_request(url, request).await.map_err(|e| {
            DepositVerificationError::HttpError(format!("Failed to send request for watch runes deposit: {}", e))
        })
    }

    async fn watch_spark_deposit(
        &self,
        request: WatchSparkDepositRequest,
    ) -> Result<WatchSparkDepositResponse, DepositVerificationError> {
        let url = self.get_url(WATCH_SPARK_DEPOSIT_PATH).await.map_err(|e| {
            DepositVerificationError::InvalidRequest(format!("Failed to get URL for watch spark deposit: {}", e))
        })?;

        self.send_post_json_request(url, request).await.map_err(|e| {
            DepositVerificationError::HttpError(format!("Failed to send request for watch spark deposit: {}", e))
        })
    }
}
