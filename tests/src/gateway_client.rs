use reqwest::Client;
use url::Url;
use thiserror::Error;
use bitcoin::Address;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Clone, Debug)]
pub struct GatewayClient {
    client: Client,
    config: GatewayConfig,
}

#[derive(Clone, Debug)]
pub struct GatewayConfig {
    pub address: Url,
}

#[derive(Error, Debug)]
pub enum GatewayClientError {
    #[error("Failed to join URL: {0}")]
    UrlJoinError(#[from] url::ParseError),
    #[error("Failed to send request: {0}")]
    SendRequestError(#[from] reqwest::Error),
    #[error("Error response: {0}")]
    ErrorResponse(String),
}

const GET_RUNES_DEPOSIT_ADDRESS_PATH: &str = "/api/user/get-btc-deposit-address";

#[derive(Serialize, Debug)]
pub struct GetRunesDepositAddressRequest {
    pub user_public_key: String,
    pub rune_id: String,
    pub amount: u64,
}

#[derive(Deserialize, Debug)]
pub struct GetRunesDepositAddressResponse {
    pub address: String,
}

const TEST_SPARK_PATH: &str = "/api/test/test-spark";

#[derive(Serialize, Debug)]
pub struct TestSparkRequest {
    pub btc_address: String,
}

#[derive(Deserialize, Debug)]
pub struct TestSparkResponse {}

impl GatewayClient {
    pub fn new(config: GatewayConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub async fn send_request<T: Serialize, U: DeserializeOwned>(&self, address_path: &str, request: T) -> Result<U, GatewayClientError> {
        let url = self.config.address.join(address_path)?;

        let response = self.client.post(url).json(&request).send().await?;

        if response.status().is_success() {
            let response: U = response.json().await?;
            Ok(response)
        } else {
            Err(GatewayClientError::ErrorResponse(format!("Error response with status: {}", response.status())))
        }
    }

    pub async fn get_runes_deposit_address(&self, request: GetRunesDepositAddressRequest) -> Result<GetRunesDepositAddressResponse, GatewayClientError> {
        self.send_request(GET_RUNES_DEPOSIT_ADDRESS_PATH, request).await
    }

    pub async fn test_spark(&self, request: TestSparkRequest) -> Result<TestSparkResponse, GatewayClientError> {
        self.send_request(TEST_SPARK_PATH, request).await
    }
}
