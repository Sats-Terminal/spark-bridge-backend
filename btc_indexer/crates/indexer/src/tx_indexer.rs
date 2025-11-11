use crate::error::IndexerError;
use btc_indexer_client::client_api::BtcIndexer;
use btc_indexer_config::BtcIndexerConfig;
use btc_indexer_local_db_store::schemas::txs::{TxInfo, TxsStorage};
use btc_indexer_local_db_store::storage::LocalDbStorage;
use std::sync::Arc;
use tokio::select;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing;

pub struct TxIndexer<Api: BtcIndexer> {
    indexer_client: Api,
    local_db_store: Arc<LocalDbStorage>,
    cancellation_token: CancellationToken,
    config: BtcIndexerConfig,
    cur_block_height: u64,
}

impl<Api: BtcIndexer> TxIndexer<Api> {
    pub fn new(
        indexer_client: Api,
        local_db_store: Arc<LocalDbStorage>,
        cancellation_token: CancellationToken,
        config: BtcIndexerConfig,
    ) -> Self {
        let cur_block_height = config.start_block_height;
        Self {
            indexer_client,
            local_db_store,
            cancellation_token,
            config,
            cur_block_height,
        }
    }

    pub async fn run(&mut self) -> Result<(), IndexerError> {
        tracing::info!("Tx indexer running");
        loop {
            select! {
                _ = self.cancellation_token.cancelled() => {
                    return Ok(());
                }
                _ = tokio::time::sleep(Duration::from_millis(self.config.tx_indexer_update_interval_millis)) => {
                    tracing::debug!("Getting new transactions");
                    let _ = self.get_new_transactions().await
                        .inspect_err(|e| tracing::error!("Error processing txs: {:?}", e));
                }
            }
        }
    }

    async fn get_new_transactions(&mut self) -> Result<(), IndexerError> {
        let new_cur_block_height = self.indexer_client.get_blockchain_info().await?.block_height;

        if new_cur_block_height > self.cur_block_height {
            tracing::debug!(
                "Getting new transactions from block height {} to {}",
                self.cur_block_height + 1,
                new_cur_block_height
            );
            for block_height in (self.cur_block_height + 1)..=new_cur_block_height {
                let transactions = self.indexer_client.get_block_transactions(block_height).await?;
                let transacion_infos = transactions
                    .into_iter()
                    .map(|txid| TxInfo { txid, block_height })
                    .collect::<Vec<TxInfo>>();
                self.local_db_store.insert_txs(transacion_infos).await?;
            }

            self.cur_block_height = new_cur_block_height;
        }

        Ok(())
    }
}
