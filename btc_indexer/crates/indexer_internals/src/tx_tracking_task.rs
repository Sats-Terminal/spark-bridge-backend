use crate::indexer::BtcIndexer;
use config_parser::config::BtcIndexerParams;
use local_db_store_indexer::init::LocalDbStorage;
use local_db_store_indexer::schemas::runes_spark::tx_ids_indexed::TxIdIndexed;
use persistent_storage::error::DbError;
use persistent_storage::init::PersistentRepoTrait;
use std::sync::Arc;
use std::time::Duration;
use titan_client::{TitanApi, TitanClient};
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, trace};

const ACCOUNT_TRACKING_LOG_PATH: &str = "btc_indexer:account_tracking";

// TODO: maybe move to usage of traits for db interaction but not trait which returns ref to postgres
pub fn spawn<C: TitanApi, Db: PersistentRepoTrait + Clone + Send + Sync + 'static>(
    cancellation_token: CancellationToken,
    local_db: Arc<Db>,
    btc_indexer_params: BtcIndexerParams,
    titan_client: Arc<BtcIndexer<C, Db>>,
) {
    tokio::spawn({
        let mut interval = tokio::time::interval(Duration::from_millis(btc_indexer_params.update_interval_millis));
        let local_db = local_db.clone();
        async move {
            // todo: save about user that begun transaction to renew connection in bad cases
            //  change status on processing
            trace!("[{ACCOUNT_TRACKING_LOG_PATH}] Loop spawned..");
            'checking_loop: loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        debug!("[{ACCOUNT_TRACKING_LOG_PATH}] Closing [Btc indexer] account updating task, because of cancellation token");
                        break 'checking_loop;
                    },
                    _ = interval.tick() => {
                        //todo: retrieve list of txs to track
                        perform_status_update(local_db.clone(), titan_client.clone()).await;
                    }
                }
            }
        }
    });
}

async fn perform_status_update<C: TitanApi, Db: PersistentRepoTrait + Clone + Send + Sync + 'static>(
    db_storage: Arc<Db>,
    titan_client: Arc<BtcIndexer<C, Db>>,
) {
    // let txs: Vec<TxIdIndexed> = get_list_of_txs_to_track(db_storage.clone()).await?;
    trace!("[{ACCOUNT_TRACKING_LOG_PATH}] tick triggered");
    // for tx_id in txs
    // match titan_client.get_transaction(&tx_id.).await {
    //     Ok(data) => {
    //         trace!("[{ACCOUNT_TRACKING_LOG_PATH}] address data successfully received, {data:?}");
    //         if !data.outputs.is_empty() && is_confirmed_outs(&data.outputs) {
    //             let _ = event_tx.send(AccountReplenishmentEvent { address: account_addr, account_data: data });
    //             // todo: save data in db about processed tx
    //             break 'checking_loop;
    //         }
    //     }
    //     Err(e) => {
    //         error!("[{ACCOUNT_TRACKING_LOG_PATH}] Failed to retrieve account data by address: {e}")
    //     }
    // };
    todo!()
}

async fn get_list_of_txs_to_track<Db: PersistentRepoTrait + Clone + Send + Sync>(
    db_storage: Arc<Db>,
) -> Result<Vec<TxIdIndexed>, DbError> {
    todo!()
}
