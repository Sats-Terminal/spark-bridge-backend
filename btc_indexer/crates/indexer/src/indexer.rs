use crate::callback_client::CallbackClient;
use btc_indexer_config::BtcIndexerConfig;
use btc_indexer_client::client_api::BtcIndexerClientApi;
use tokio::time::Duration;
use btc_indexer_local_db_store::storage::LocalDbStorage;
use btc_indexer_local_db_store::schemas::requests::{WatchRequestErrorDetails, WatchRequestStatus, WatchRequest, ValidationResult, RequestsStorage};
use btc_indexer_local_db_store::schemas::txs::TxsStorage;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tokio::select;
use crate::error::IndexerError;
use btc_indexer_client::client_api::BlockchainInfo;
use crate::callback_client::NotifyRunesDepositRequest;
use btc_indexer_client::client_api::OutPointData;
use tracing::instrument;
use ordinals::RuneId;
use crate::callback_client::DepositStatus;
use chrono::Utc;

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
                _ = tokio::time::sleep(Duration::from_millis(self.config.indexer_update_interval_millis)) => {
                    let _ = self.process_watch_requests().await
                        .inspect_err(|e| tracing::error!("Error processing watch requests: {:?}", e));
                }
            }
        }
    }

    async fn process_watch_requests(&self) -> Result<(), IndexerError> {
        tracing::info!("Processing watch requests");
        let watch_requests = self.local_db_store.get_all_unprocessed_watch_requests().await?;
        tracing::debug!("Found {} watch requests to process", watch_requests.len());
        tracing::debug!("Watch requests: {:?}", watch_requests);
        for watch_request in watch_requests {
            let blockchain_info = self.indexer_client.get_blockchain_info().await?;
            if self.local_db_store.exists(watch_request.outpoint.txid).await? {
                let outpoint_data = self.indexer_client.get_transaction_outpoint(watch_request.outpoint.clone()).await?
                    .ok_or(IndexerError::InvalidData("Outpoint data not found".to_string()))?;

                let (validation_result, validation_metadata) = self.validate_deposit_transaction(watch_request.clone(), outpoint_data.clone(), blockchain_info.clone()).await?;
                
                match validation_result.watch_request_status {
                    WatchRequestStatus::Confirmed | WatchRequestStatus::Failed => {
                        self.local_db_store.update_watch_request_status(watch_request.id, validation_result.clone()).await?;
                        self.send_notify_request(watch_request.clone(), validation_result.clone(), validation_metadata.clone()).await?;
                    }
                    WatchRequestStatus::Pending => continue,
                }
            } else {
                let (validation_result, validation_metadata) = self.validate_transaction_timeout(watch_request.clone()).await?;
                match validation_result.watch_request_status {
                    WatchRequestStatus::Confirmed => {
                        tracing::error!("Invalid watch request status confirmed");
                        return Err(IndexerError::InvalidData("Invalid watch request status confirmed".to_string()));
                    }
                    WatchRequestStatus::Failed => {
                        tracing::error!("Transaction timeout for outpoint: {}", watch_request.outpoint);
                        self.local_db_store.update_watch_request_status(watch_request.id, validation_result.clone()).await?;
                        self.send_notify_request(watch_request.clone(), validation_result.clone(), validation_metadata.clone()).await?;
                    }
                    WatchRequestStatus::Pending => continue,
                };
            }
        }
        Ok(())
    }

    async fn validate_transaction_timeout(&self, watch_request: WatchRequest) -> Result<(ValidationResult, Option<ValidationMetadata>), IndexerError> {
        let cur_timestamp = Utc::now();
        if Duration::from_millis((cur_timestamp.timestamp_millis() as u64).saturating_sub(watch_request.created_at.timestamp_millis() as u64)) > Duration::from_millis(self.config.validation_timeout_millis) {
            tracing::error!("Timeout waiting for transaction output for outpoint: {}", watch_request.outpoint);
            return Ok((ValidationResult { 
                watch_request_status: WatchRequestStatus::Failed, 
                error_details: Some(WatchRequestErrorDetails::Timeout(format!("Timeout waiting for transaction output for outpoint: {}", watch_request.outpoint))) 
            }, None))
        }

        Ok((ValidationResult { 
            watch_request_status: WatchRequestStatus::Pending, 
            error_details: None
        }, None))
    }

    #[instrument(level = "trace", skip(self, outpoint_data), ret)]
    async fn validate_deposit_transaction(&self, watch_request: WatchRequest, outpoint_data: OutPointData, blockchain_info: BlockchainInfo) -> Result<(ValidationResult, Option<ValidationMetadata>), IndexerError> {
        if blockchain_info.block_height.saturating_sub(outpoint_data.block_height) < self.config.confirmation_block_height_delta {
            tracing::debug!("Blockchain height is not confirmed yet for outpoint: {}", watch_request.outpoint);
            return Ok((ValidationResult { 
                watch_request_status: WatchRequestStatus::Pending, 
                error_details: None
            }, None))
        }

        let mut validation_metadata = ValidationMetadata::default();

        if let Some(rune_id) = watch_request.rune_id {
            let rune_amount = outpoint_data.rune_amounts.get(&rune_id).unwrap_or(&0).clone();
            let expected_rune_amount = watch_request.rune_amount.unwrap_or(0);
            if rune_amount != expected_rune_amount {
                tracing::error!("Invalid rune amount: expected: {}, got: {}", expected_rune_amount, rune_amount);
                return Ok((ValidationResult { 
                    watch_request_status: WatchRequestStatus::Failed, 
                    error_details: Some(WatchRequestErrorDetails::InvalidRuneAmount(format!("Invalid rune amount: expected: {}, got: {}", expected_rune_amount, rune_amount))) 
                }, None))
            } else {
                validation_metadata.rune_id = Some(rune_id);
                validation_metadata.rune_amount = Some(rune_amount);
            }
        }
        
        if let Some(expected_sats_amount) = watch_request.sats_amount {
            let sats_amount = outpoint_data.sats_amount;
            if sats_amount != expected_sats_amount {
                tracing::error!("Invalid sats amount: expected: {}, got: {}", expected_sats_amount, sats_amount);
                return Ok((ValidationResult { 
                    watch_request_status: WatchRequestStatus::Failed, 
                    error_details: Some(WatchRequestErrorDetails::InvalidSatsAmount(format!("Invalid sats amount: expected: {}, got: {}", expected_sats_amount, sats_amount))) 
                }, None))
            } else {
                validation_metadata.sats_amount = Some(sats_amount);
            }
        }

        // FIXME: at present verifier needs to know sats amount 
        validation_metadata.sats_amount = Some(outpoint_data.sats_amount);

        tracing::info!("Watch request validated for outpoint: {}", watch_request.outpoint);

        Ok((ValidationResult { watch_request_status: WatchRequestStatus::Confirmed, error_details: None }, Some(validation_metadata)))
    }

    async fn send_notify_request(&self, watch_request: WatchRequest, validation_result: ValidationResult, validation_metadata: Option<ValidationMetadata>) -> Result<(), IndexerError> {
        match validation_result.watch_request_status {
            WatchRequestStatus::Confirmed => {
                let validation_metadata = validation_metadata.ok_or(IndexerError::InvalidData("Validation metadata not found".to_string()))?;
                tracing::info!("Sending notify request for confirmed watch request for outpoint: {}", watch_request.outpoint);
                self.callback_client.send_notify_request(NotifyRunesDepositRequest {
                    request_id: watch_request.request_id,
                    outpoint: watch_request.outpoint,
                    deposit_status: DepositStatus::Confirmed,
                    sats_amount: validation_metadata.sats_amount,
                    rune_id: validation_metadata.rune_id,
                    rune_amount: validation_metadata.rune_amount,
                    error_details: validation_result.error_details.map(|e| e.to_string()),
                }, watch_request.callback_url).await?;
            }
            WatchRequestStatus::Failed => {
                tracing::error!("Sending notify request for failed watch request for outpoint: {}", watch_request.outpoint);
                self.callback_client.send_notify_request(NotifyRunesDepositRequest {
                    request_id: watch_request.request_id,
                    outpoint: watch_request.outpoint,
                    deposit_status: DepositStatus::Failed,
                    sats_amount: None,
                    rune_id: None,
                    rune_amount: None,
                    error_details: validation_result.error_details.map(|e| e.to_string()),
                }, watch_request.callback_url).await?;
            }
            WatchRequestStatus::Pending => {
                tracing::error!("Watch request is pending for outpoint: {}", watch_request.outpoint);
                return Err(IndexerError::InvalidData("Watch request status is pending".to_string()));
            },
        };

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ValidationMetadata {
    pub sats_amount: Option<u64>,
    pub rune_id: Option<RuneId>,
    pub rune_amount: Option<u128>,
}

impl Default for ValidationMetadata {
    fn default() -> Self {
        Self {
            sats_amount: None,
            rune_id: None,
            rune_amount: None,
        }
    }
}
