use crate::client::VerifierClient;
use async_trait::async_trait;
use bitcoin::OutPoint;
use frost::types::TweakBytes;
use gateway_deposit_verification::error::DepositVerificationError;
use gateway_deposit_verification::traits::{
    VerificationClient,
};
use gateway_deposit_verification::types::{
    WatchRunesDepositRequest, WatchRunesDepositResponse, WatchSparkDepositRequest, WatchSparkDepositResponse,
};
use gateway_local_db_store::schemas::deposit_address::DepositStatus;
use gateway_local_db_store::schemas::user_identifier::UserIds;
use serde::{Deserialize, Serialize};
use token_identifier::TokenIdentifier;
use tracing::instrument;

const WATCH_RUNES_DEPOSIT_PATH: &str = "/api/gateway/watch-runes-deposit";
const WATCH_SPARK_DEPOSIT_PATH: &str = "/api/gateway/watch-spark-deposit";

#[derive(Serialize, Debug)]
pub struct VerifierWatchRunesDepositRequest {
    pub user_ids: UserIds,
    pub nonce: TweakBytes,
    pub amount: u64,
    pub btc_address: String,
    pub bridge_address: String,
    pub out_point: OutPoint,
}

impl From<WatchRunesDepositRequest> for VerifierWatchRunesDepositRequest {
    fn from(request: WatchRunesDepositRequest) -> Self {
        Self {
            user_ids: request.user_ids,
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

impl From<VerifierWatchRunesDepositResponse> for WatchRunesDepositResponse {
    fn from(_value: VerifierWatchRunesDepositResponse) -> Self {
        WatchRunesDepositResponse {}
    }
}

#[derive(Debug, Serialize)]
pub struct VerifierWatchSparkDepositRequest {
    pub user_ids: UserIds,
    pub nonce: TweakBytes,
    pub exit_address: String,
    pub amount: u64,
    pub spark_address: String,
    pub token_identifier: TokenIdentifier,
}

impl From<WatchSparkDepositRequest> for VerifierWatchSparkDepositRequest {
    fn from(request: WatchSparkDepositRequest) -> Self {
        Self {
            user_ids: request.user_ids,
            nonce: request.nonce,
            exit_address: request.exit_address.to_string(),
            amount: request.amount,
            spark_address: request.spark_address,
            token_identifier: request.token_identifier,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct VerifierWatchSparkDepositResponse {
    pub verifier_response: DepositStatus,
}

impl From<VerifierWatchSparkDepositResponse> for WatchSparkDepositResponse {
    fn from(value: VerifierWatchSparkDepositResponse) -> Self {
        WatchSparkDepositResponse {
            verifier_response: value.verifier_response,
        }
    }
}

#[async_trait]
impl VerificationClient for VerifierClient {
    #[instrument(level = "trace", skip(self), ret)]
    async fn watch_runes_deposit(
        &self,
        request: WatchRunesDepositRequest,
    ) -> Result<WatchRunesDepositResponse, DepositVerificationError> {
        let url = self.get_url(WATCH_RUNES_DEPOSIT_PATH).await.map_err(|e| {
            DepositVerificationError::InvalidRequest(format!("Failed to get URL for watch runes deposit: {}", e))
        })?;

        let request: VerifierWatchRunesDepositRequest = request.into();

        let response: VerifierWatchRunesDepositResponse =
            self.send_post_json_request(url, request).await.map_err(|e| {
                DepositVerificationError::HttpError(format!("Failed to send request for watch runes deposit: {}", e))
            })?;

        Ok(response.into())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn watch_spark_deposit(
        &self,
        request: WatchSparkDepositRequest,
    ) -> Result<WatchSparkDepositResponse, DepositVerificationError> {
        let url = self.get_url(WATCH_SPARK_DEPOSIT_PATH).await.map_err(|e| {
            DepositVerificationError::InvalidRequest(format!("Failed to get URL for watch spark deposit: {}", e))
        })?;

        let request: VerifierWatchSparkDepositRequest = request.into();

        let response: VerifierWatchSparkDepositResponse =
            self.send_post_json_request(url, request).await.map_err(|e| {
                DepositVerificationError::HttpError(format!("Failed to send request for watch spark deposit: {}", e))
            })?;

        Ok(response.into())
    }
}
