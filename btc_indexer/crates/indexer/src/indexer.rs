use crate::callback_client::CallbackClient;
use btc_indexer_config::BtcIndexerConfig;
use btc_indexer_client::client_api::BtcIndexerClientApi;
use tokio::time::Duration;
use btc_indexer_local_db_store::LocalDbStorage;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tokio::select;
use crate::error::IndexerError;
use btc_indexer_local_db_store::{WatchRequestErrorDetails, WatchRequestStatus};
use btc_indexer_local_db_store::WatchRequest;
use chrono::Utc;
use btc_indexer_local_db_store::ValidationResult;
use crate::callback_client::NotifyRequest;
use btc_indexer_client::client_api::OutPointData;

pub struct Indexer<Api: BtcIndexerClientApi> {
    callback_client: CallbackClient,
    config: BtcIndexerConfig,
    indexer_client: Api,
    local_db_store: Arc<LocalDbStorage>,
    cancellation_token: CancellationToken,
}

impl<Api: BtcIndexerClientApi> Indexer<Api> {
    pub fn new(
        config: BtcIndexerConfig,
        indexer_client: Api,
        local_db_store: Arc<LocalDbStorage>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self { 
            callback_client: CallbackClient::new(), 
            config, 
            indexer_client, 
            local_db_store, 
            cancellation_token 
        }
    }

    pub async fn run(&self) -> Result<(), IndexerError> {
        tracing::info!("Indexer running");
        loop {
            select! {
                _ = self.cancellation_token.cancelled() => {
                    return Ok(());
                }
                _ = tokio::time::sleep(Duration::from_millis(self.config.update_interval_millis)) => {
                    self.process_watch_requests().await
                        .inspect_err(|e| tracing::error!("Error processing watch requests: {:?}", e))?;
                }
            }
        }
    }

    async fn process_watch_requests(&self) -> Result<(), IndexerError> {
        tracing::info!("Processing watch requests");
        let watch_requests = self.local_db_store.get_all_unprocessed_watch_requests().await?;
        for watch_request in watch_requests {
            let outpoint_data = self.indexer_client.get_transaction_outpoint(watch_request.outpoint.clone()).await?;

            let validation_result = self.validate_watch_request(watch_request.clone(), outpoint_data.clone()).await?;
            
            match validation_result.watch_request_status {
                WatchRequestStatus::Confirmed | WatchRequestStatus::Failed => {
                    self.local_db_store.update_watch_request_status(watch_request.outpoint, validation_result.clone()).await?;
                }
                WatchRequestStatus::Pending => continue,
            }

            match validation_result.watch_request_status {
                WatchRequestStatus::Confirmed => {
                    self.callback_client.send_notify_request(NotifyRequest {
                        outpoint: watch_request.outpoint,
                        status: WatchRequestStatus::Confirmed,
                        sats_amount: watch_request.sats_amount,
                        rune_id: watch_request.rune_id,
                        rune_amount: watch_request.rune_amount,
                        error_details: validation_result.error_details,
                    }, watch_request.callback_url).await?;
                }
                WatchRequestStatus::Failed => {
                    self.callback_client.send_notify_request(NotifyRequest {
                        outpoint: watch_request.outpoint,
                        status: WatchRequestStatus::Failed,
                        sats_amount: None,
                        rune_id: None,
                        rune_amount: None,
                        error_details: validation_result.error_details,
                    }, watch_request.callback_url).await?;
                }
                WatchRequestStatus::Pending => {},
            }
        }
        Ok(())
    }

    async fn validate_watch_request(&self, watch_request: WatchRequest, outpoint_data: Option<OutPointData>) -> Result<ValidationResult, IndexerError> {
        let outpoint_data = match outpoint_data {
            Some(outpoint_data) => outpoint_data,
            None => {
                tracing::warn!("No outpoint data found for outpoint: {}", watch_request.outpoint);
                let cur_timestamp = get_cur_timestamp();
                if cur_timestamp.saturating_sub(watch_request.created_at) > self.config.validation_timeout_millis {
                    tracing::error!("Timeout waiting for transaction output for outpoint: {}", watch_request.outpoint);
                    return Ok(ValidationResult { 
                        watch_request_status: WatchRequestStatus::Failed, 
                        error_details: Some(WatchRequestErrorDetails::Timeout(format!("Timeout waiting for transaction output for outpoint: {}", watch_request.outpoint))) 
                    })
                }
                return Ok(ValidationResult { 
                    watch_request_status: WatchRequestStatus::Pending, 
                    error_details: None
                })
            }
        };

        let blockchain_info = self.indexer_client.get_blockchain_info().await?;

        if outpoint_data.block_height.saturating_sub(blockchain_info.block_height) < self.config.confirmation_block_height_delta {
            return Ok(ValidationResult { 
                watch_request_status: WatchRequestStatus::Pending, 
                error_details: None
            })
        }

        if let Some(rune_id) = watch_request.rune_id {
            let rune_amount = outpoint_data.rune_amounts.get(&rune_id).unwrap_or(&0).clone();
            let expected_rune_amount = watch_request.rune_amount.unwrap_or(0);
            if rune_amount != expected_rune_amount {
                tracing::error!("Invalid rune amount: expected: {}, got: {}", expected_rune_amount, rune_amount);
                return Ok(ValidationResult { 
                    watch_request_status: WatchRequestStatus::Failed, 
                    error_details: Some(WatchRequestErrorDetails::InvalidRuneAmount { expected: expected_rune_amount, got: rune_amount }) 
                })
            }
        }
        
        if let Some(expected_sats_amount) = watch_request.sats_amount {
            let sats_amount = outpoint_data.sats_amount;
            if sats_amount != expected_sats_amount {
                tracing::error!("Invalid sats amount: expected: {}, got: {}", expected_sats_amount, sats_amount);
                return Ok(ValidationResult { 
                    watch_request_status: WatchRequestStatus::Failed, 
                    error_details: Some(WatchRequestErrorDetails::InvalidSatsAmount { expected: expected_sats_amount, got: sats_amount }) 
                })
            }
        }

        tracing::info!("Watch request validated for outpoint: {}", watch_request.outpoint);

        Ok(ValidationResult { watch_request_status: WatchRequestStatus::Confirmed, error_details: None })
    }
}

fn get_cur_timestamp() -> u64 {
    Utc::now().timestamp_millis() as u64
}
