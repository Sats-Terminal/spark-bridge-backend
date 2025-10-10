use reqwest::Client;
use crate::error::IndexerError;
use serde::Serialize;
use url::Url;
use btc_indexer_local_db_store::WatchRequestStatus;
use bitcoin::OutPoint;
use ordinals::RuneId;
use btc_indexer_local_db_store::WatchRequestErrorDetails;

#[derive(Debug, Clone)]
pub struct CallbackClient {
    client: Client,
}

#[derive(Serialize, Debug, Clone)]
pub struct NotifyRequest {
    pub outpoint: OutPoint,
    pub status: WatchRequestStatus,
    pub sats_amount: Option<u64>,
    pub rune_id: Option<RuneId>,
    pub rune_amount: Option<u128>,
    pub error_details: Option<WatchRequestErrorDetails>,
}

impl CallbackClient {
    pub fn new() -> Self {
        Self { client: Client::new() }
    }

    pub async fn send_callback_request<T: Serialize>(&self, request: T, callback_url: Url) -> Result<(), IndexerError> {
        let response = self.client
            .post(callback_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| IndexerError::CallbackClientError(format!("Failed to send callback request: {:?}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let error_message = response.text().await.unwrap_or_default().to_string();
            tracing::error!("Failed to send callback request: {:?}", error_message.clone());
            Err(IndexerError::CallbackClientError(format!("Failed to send callback request: {:?}", error_message)))
        }
    }

    pub async fn send_notify_request(&self, request: NotifyRequest, callback_url: Url) -> Result<(), IndexerError> {
        self.send_callback_request(request, callback_url).await
    }
}
