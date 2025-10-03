use crate::error::BtcIndexerClientError;
use bitcoin::OutPoint;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use url::Url;
use verifier_config_parser::config::BtcIndexerConfig;

const WATCH_RUNES_DEPOSIT_PATH: &str = "/track_tx";

#[derive(Serialize, Deserialize, Debug)]
pub struct WatchRunesDepositRequest {
    pub btc_address: String,
    pub out_point: OutPoint,
    pub rune_id: String,
    pub rune_amount: u64,
    pub callback_url: Url,
}

#[derive(Clone, Debug)]
pub struct BtcIndexerClient {
    config: BtcIndexerConfig,
    client: Client,
}

impl BtcIndexerClient {
    pub fn new(config: BtcIndexerConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub async fn watch_runes_deposit(&self, request: WatchRunesDepositRequest) -> Result<(), BtcIndexerClientError> {
        let url = self
            .config
            .address
            .join(WATCH_RUNES_DEPOSIT_PATH)
            .map_err(|e| BtcIndexerClientError::DeserializeError(format!("Failed to join URL: {:?}", e)))?;
        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|e| BtcIndexerClientError::HttpError(format!("Failed to send request: {:?}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            tracing::error!(
                "Failed to send HTTP request for {:?}, with status {}",
                request,
                response.status()
            );
            Err(BtcIndexerClientError::HttpError(format!(
                "Failed to send HTTP request with status {}, error: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )))
        }
    }
}
