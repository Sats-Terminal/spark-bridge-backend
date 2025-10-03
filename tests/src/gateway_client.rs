use crate::error::GatewayClientError;
use bitcoin::secp256k1::schnorr::Signature;
use reqwest::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use url::Url;

#[derive(Clone, Debug)]
pub struct GatewayClient {
    client: Client,
    config: GatewayConfig,
}

#[derive(Clone, Debug)]
pub struct GatewayConfig {
    pub address: Url,
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

const GET_SPARK_DEPOSIT_ADDRESS_PATH: &str = "/api/user/get-spark-deposit-address";

#[derive(Serialize, Debug)]
pub struct GetSparkDepositAddressRequest {
    pub user_public_key: String,
    pub rune_id: String,
    pub amount: u64,
}

#[derive(Deserialize, Debug)]
pub struct GetSparkDepositAddressResponse {
    pub address: String,
}

const BRIDGE_RUNES_PATH: &str = "/api/user/bridge-runes";

#[derive(Serialize, Debug)]
pub struct BridgeRunesSparkRequest {
    pub btc_address: String,
    pub bridge_address: String,
    pub txid: String,
    pub vout: u32,
}

pub type BridgeRunesSparkResponse = ();

const EXIT_SPARK_PATH: &str = "/api/user/exit-spark";

#[derive(Serialize, Debug)]
pub struct ExitSparkRequest {
    pub spark_address: String,
    pub paying_input: UserPayingTransferInput,
}

#[derive(Serialize, Debug)]
pub struct UserPayingTransferInput {
    pub txid: String,
    pub vout: u32,
    pub btc_exit_address: String,
    pub sats_amount: u64,
    pub none_anyone_can_pay_signature: Signature,
}

pub type ExitSparkResponse = ();

impl GatewayClient {
    pub fn new(config: GatewayConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub async fn send_request<T: Serialize, U: DeserializeOwned>(
        &self,
        address_path: &str,
        request: T,
    ) -> Result<U, GatewayClientError> {
        let url = self.config.address.join(address_path)?;

        let response = self.client.post(url).json(&request).send().await?;

        if response.status().is_success() {
            let response: U = response.json().await?;
            Ok(response)
        } else {
            Err(GatewayClientError::ErrorResponse(format!(
                "Error response with status: {}",
                response.status()
            )))
        }
    }

    pub async fn get_runes_deposit_address(
        &self,
        request: GetRunesDepositAddressRequest,
    ) -> Result<GetRunesDepositAddressResponse, GatewayClientError> {
        self.send_request(GET_RUNES_DEPOSIT_ADDRESS_PATH, request).await
    }

    pub async fn bridge_runes(
        &self,
        request: BridgeRunesSparkRequest,
    ) -> Result<BridgeRunesSparkResponse, GatewayClientError> {
        self.send_request(BRIDGE_RUNES_PATH, request).await
    }

    pub async fn get_spark_deposit_address(
        &self,
        request: GetSparkDepositAddressRequest,
    ) -> Result<GetSparkDepositAddressResponse, GatewayClientError> {
        self.send_request(GET_SPARK_DEPOSIT_ADDRESS_PATH, request).await
    }

    pub async fn exit_spark(&self, request: ExitSparkRequest) -> Result<ExitSparkResponse, GatewayClientError> {
        self.send_request(EXIT_SPARK_PATH, request).await
    }
}
