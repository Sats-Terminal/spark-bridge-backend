use crate::client::VerifierClient;
use async_trait::async_trait;
use frost::errors::AggregatorError;
use frost::traits::SignerClient;
use frost::types::{DkgFinalizeRequest, DkgFinalizeResponse};
use frost::types::{DkgRound1Request, DkgRound1Response};
use frost::types::{DkgRound2Request, DkgRound2Response};
use frost::types::{SignRound1Request, SignRound1Response};
use frost::types::{SignRound2Request, SignRound2Response};

const DKG_ROUND_1_PATH: &str = "/api/gateway/dkg-round-1";
const DKG_ROUND_2_PATH: &str = "/api/gateway/dkg-round-2";
const DKG_FINALIZE_PATH: &str = "/api/gateway/dkg-finalize";
const SIGN_ROUND_1_PATH: &str = "/api/gateway/sign-round-1";
const SIGN_ROUND_2_PATH: &str = "/api/gateway/sign-round-2";

#[async_trait]
impl SignerClient for VerifierClient {
    async fn dkg_round_1(&self, request: DkgRound1Request) -> Result<DkgRound1Response, AggregatorError> {
        let url = self
            .get_url(DKG_ROUND_1_PATH)
            .await
            .map_err(|e| AggregatorError::InvalidRequest(format!("Failed to get URL for DKG round 1: {}", e)))?;

        self.send_request(url, request)
            .await
            .map_err(|e| AggregatorError::HttpError(format!("Failed to send request for DKG round 1: {}", e)))
    }

    async fn dkg_round_2(&self, request: DkgRound2Request) -> Result<DkgRound2Response, AggregatorError> {
        let url = self
            .get_url(DKG_ROUND_2_PATH)
            .await
            .map_err(|e| AggregatorError::InvalidRequest(format!("Failed to get URL for DKG round 2: {}", e)))?;

        self.send_request(url, request)
            .await
            .map_err(|e| AggregatorError::HttpError(format!("Failed to send request for DKG round 2: {}", e)))
    }

    async fn dkg_finalize(&self, request: DkgFinalizeRequest) -> Result<DkgFinalizeResponse, AggregatorError> {
        let url = self
            .get_url(DKG_FINALIZE_PATH)
            .await
            .map_err(|e| AggregatorError::InvalidRequest(format!("Failed to get URL for DKG finalize: {}", e)))?;

        self.send_request(url, request)
            .await
            .map_err(|e| AggregatorError::HttpError(format!("Failed to send request for DKG finalize: {}", e)))
    }

    async fn sign_round_1(&self, request: SignRound1Request) -> Result<SignRound1Response, AggregatorError> {
        let url = self
            .get_url(SIGN_ROUND_1_PATH)
            .await
            .map_err(|e| AggregatorError::InvalidRequest(format!("Failed to get URL for sign round 1: {}", e)))?;

        self.send_request(url, request)
            .await
            .map_err(|e| AggregatorError::HttpError(format!("Failed to send request for sign round 1: {}", e)))
    }

    async fn sign_round_2(&self, request: SignRound2Request) -> Result<SignRound2Response, AggregatorError> {
        let url = self
            .get_url(SIGN_ROUND_2_PATH)
            .await
            .map_err(|e| AggregatorError::InvalidRequest(format!("Failed to get URL for sign round 2: {}", e)))?;

        self.send_request(url, request)
            .await
            .map_err(|e| AggregatorError::HttpError(format!("Failed to send request for sign round 2: {}", e)))
    }
}
