use crate::api::ChanMsg;
use crate::indexer::BtcIndexer;
use bitcoin::Txid;
use config_parser::config::BtcIndexerParams;
use local_db_store_indexer::init::LocalDbStorage;
use local_db_store_indexer::schemas::runes_spark::tx_ids_indexed::TxIdIndexed;
use persistent_storage::error::DbError;
use persistent_storage::init::PersistentRepoTrait;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use titan_client::{TitanApi, TitanClient};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, trace};

const ACCOUNT_TRACKING_LOG_PATH: &str = "btc_indexer:account_tracking";

// TODO: maybe move to usage of traits for db interaction but not trait which returns ref to postgres
pub fn spawn<C: TitanApi, Db: PersistentRepoTrait + Clone + Send + Sync + 'static>(
    cancellation_token: CancellationToken,
    local_db: Db,
    btc_indexer_params: BtcIndexerParams,
    titan_client: C,
    mut chan_receiver: UnboundedReceiver<ChanMsg>,
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
                    x = chan_receiver.recv() => {
                        // let updated_txs = perform_status_update(local_db.clone(), titan_client.clone()).await;
                        // send_response_to_subscribers(local_db.clone(), updated_txs);
                    }
                    _ = interval.tick() => {
                        let updated_txs = perform_status_update(local_db.clone(), titan_client.clone()).await;
                        send_response_to_subscribers(local_db.clone(), updated_txs).await;
                    }
                }
            }
        }
    });
}

async fn send_response_to_subscribers<Db: PersistentRepoTrait + Clone + Send + Sync + 'static>(
    local_db: Db,
    txs_to_send: Vec<Txid>,
) {
    let tasks = spawn_tasks_to_send_response(local_db, txs_to_send).unwrap();
    //todo: map errors
    futures::future::join_all(tasks)
        .await
        .into_iter()
        .collect::<Result<Vec<()>, _>>()
        .unwrap();
}

fn spawn_tasks_to_send_response<Db: PersistentRepoTrait + Clone + Send + Sync + 'static>(
    p0: Db,
    p1: Vec<Txid>,
) -> anyhow::Result<Vec<JoinHandle<()>>> {
    todo!()
}

async fn perform_status_update<C: TitanApi, Db: PersistentRepoTrait + Clone + Send + Sync + 'static>(
    db_storage: Db,
    titan_client: C,
) -> Vec<Txid> {
    let txs: Vec<TxIdIndexed> = get_list_of_txs_to_track(db_storage.clone()).await.unwrap();
    trace!("[{ACCOUNT_TRACKING_LOG_PATH}] tick triggered");
    run_tasks_to_check_txs(txs, titan_client, db_storage).await
}

async fn run_tasks_to_check_txs<C: TitanApi, Db: PersistentRepoTrait + Clone + Send + Sync + 'static>(
    checked_txs: Vec<TxIdIndexed>,
    titan_client: C,
    db_storage: Db,
) -> Vec<Txid> {
    let mut check_txs_tasks: Vec<JoinHandle<Txid>> = Vec::with_capacity(checked_txs.len());
    for tx_id in checked_txs {
        match titan_client.get_transaction(&tx_id.tx_id.0).await {
            Ok(data) => {
                check_txs_tasks.push(create_check_tx_task());
            }
            Err(e) => {
                error!("[{ACCOUNT_TRACKING_LOG_PATH}] Failed to retrieve account data by address: {e}")
            }
        };
    }
    futures::future::join_all(check_txs_tasks)
        .await
        .into_iter()
        .collect::<Result<Vec<Txid>, _>>()
        .unwrap()
}

fn create_check_tx_task() -> JoinHandle<Txid> {
    tokio::spawn(async move {
        trace!("[{ACCOUNT_TRACKING_LOG_PATH}] address data successfully received, ");
        // if !data.outputs.is_empty() && is_confirmed_outs(&data.outputs) {
        //     let _ = event_tx.send(AccountReplenishmentEvent { address: account_addr, account_data: data });
        //     // todo: save data in db about processed tx
        // }
        //todo: some logic to check tx
        Txid::from_str("hello").unwrap()
    })
}

async fn get_list_of_txs_to_track<Db: PersistentRepoTrait + Clone + Send + Sync>(
    db_storage: Db,
) -> Result<Vec<TxIdIndexed>, DbError> {
    todo!()
}
