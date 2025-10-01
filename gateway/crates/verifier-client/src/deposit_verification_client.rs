use crate::client::VerifierClient;
use async_trait::async_trait;
use gateway_deposit_verification::error::DepositVerificationError;
use gateway_deposit_verification::traits::VerificationClient;
use gateway_deposit_verification::types::{
    WatchRunesDepositRequest, WatchRunesDepositResponse, WatchSparkDepositRequest, WatchSparkDepositResponse,
};
use gateway_local_db_store::schemas::deposit_address::DepositStatus;
use serde::{Deserialize, Serialize};
use bitcoin::OutPoint;
use frost::types::MusigId;
use frost::types::Nonce;

const WATCH_RUNES_DEPOSIT_PATH: &str = "/api/gateway/watch-runes-deposit";
const WATCH_SPARK_DEPOSIT_PATH: &str = "/api/gateway/watch-spark-deposit";

#[derive(Serialize, Debug)]
pub struct VerifierWatchRunesDepositRequest {
    pub musig_id: MusigId,
    pub nonce: Nonce,
    pub amount: u64,
    pub btc_address: String,
    pub bridge_address: String,
    pub out_point: OutPoint,
}

impl From<WatchRunesDepositRequest> for VerifierWatchRunesDepositRequest {
    fn from(request: WatchRunesDepositRequest) -> Self {
        Self {
            musig_id: request.musig_id,
            nonce: request.nonce,
            amount: request.amount,
            btc_address: request.btc_address.to_string(),
            bridge_address: request.bridge_address,
            out_point: request.out_point,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct VerifierWatchRunesDepositResponse {}

impl Into<WatchRunesDepositResponse> for VerifierWatchRunesDepositResponse {
    fn into(self) -> WatchRunesDepositResponse {
        WatchRunesDepositResponse {}
    }
}

#[derive(Debug, Serialize)]
pub struct VerifierWatchSparkDepositRequest {
    pub musig_id: MusigId,
    pub nonce: Nonce,
    pub exit_address: String,
    pub amount: u64,
    pub spark_address: String,
}

impl From<WatchSparkDepositRequest> for VerifierWatchSparkDepositRequest {
    fn from(request: WatchSparkDepositRequest) -> Self {
        Self {
            musig_id: request.musig_id,
            nonce: request.nonce,
            exit_address: request.exit_address.to_string(),
            amount: request.amount,
            spark_address: request.spark_address,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct VerifierWatchSparkDepositResponse {
    pub verifier_response: DepositStatus,
}

impl Into<WatchSparkDepositResponse> for VerifierWatchSparkDepositResponse {
    fn into(self) -> WatchSparkDepositResponse {
        WatchSparkDepositResponse {
            verifier_response: self.verifier_response,
        }
    }
}

#[async_trait]
impl VerificationClient for VerifierClient {
    async fn watch_runes_deposit(
        &self,
        request: WatchRunesDepositRequest,
    ) -> Result<WatchRunesDepositResponse, DepositVerificationError> {
        let url = self.get_url(WATCH_RUNES_DEPOSIT_PATH).await.map_err(|e| {
            DepositVerificationError::InvalidRequest(format!("Failed to get URL for watch runes deposit: {}", e))
        })?;

        let request: VerifierWatchRunesDepositRequest = request.into();

        let response: VerifierWatchRunesDepositResponse = self.send_request(url, request).await.map_err(|e| {
            DepositVerificationError::HttpError(format!("Failed to send request for watch runes deposit: {}", e))
        })?;

        Ok(response.into())
    }

    async fn watch_spark_deposit(
        &self,
        request: WatchSparkDepositRequest,
    ) -> Result<WatchSparkDepositResponse, DepositVerificationError> {
        let url = self.get_url(WATCH_SPARK_DEPOSIT_PATH).await.map_err(|e| {
            DepositVerificationError::InvalidRequest(format!("Failed to get URL for watch spark deposit: {}", e))
        })?;

        let request: VerifierWatchSparkDepositRequest = request.into();

        let response: VerifierWatchSparkDepositResponse = self.send_request(url, request).await.map_err(|e| {
            DepositVerificationError::HttpError(format!("Failed to send request for watch spark deposit: {}", e))
        })?;

        Ok(response.into())
    }
}
