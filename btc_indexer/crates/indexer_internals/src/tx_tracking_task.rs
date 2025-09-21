use crate::indexer::BtcIndexer;
use crate::tx_arbiter::{TxArbiter, TxArbiterResponse, TxArbiterTrait};
use anyhow::{anyhow, bail};
use bitcoin::Txid;
use btc_indexer_api::api::{BtcIndexerCallbackResponse, BtcTxReview, ResponseMeta};
use config_parser::config::BtcIndexerParams;
use local_db_store_indexer::init::{IndexerDbBounds, LocalDbStorage};
use local_db_store_indexer::schemas::track_tx_requests_storage::{TrackedReqStatus, TxTrackingRequestsToSendResponse};
use local_db_store_indexer::schemas::tx_tracking_storage::TxToUpdateStatus;
use persistent_storage::error::DbError;
use persistent_storage::init::PersistentRepoTrait;
use reqwest::{Error, Response};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use titan_client::{TitanApi, TitanClient};
use titan_types::Transaction;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, trace};

/// Msg used in thread which is responsible for updating information for existing txs
const UPDATE_TXS_INFO_LOG_PATH: &str = "btc_indexer:update_txs_info";
/// Msg used in thread which is responsible for sending msg to recipients
const FINALIZATION_TRACKING_LOG_PATH: &str = "btc_indexer:finalization_tracking";

/// Spawns tasks  [1] to track already saved txs in db, [2] to send responses to users when we have finalized tx
#[instrument(
    skip(btc_indexer_params, local_db, cancellation_token, titan_client, tx_validator),
    level = "debug"
)]
pub fn spawn<C: TitanApi, Db: IndexerDbBounds, TxValidator: TxArbiterTrait>(
    cancellation_token: CancellationToken,
    local_db: Db,
    btc_indexer_params: BtcIndexerParams,
    titan_client: C,
    tx_validator: TxValidator,
) {
    // Update txs info tracking task
    tokio::spawn({
        let mut interval = tokio::time::interval(Duration::from_millis(btc_indexer_params.update_interval_millis));
        let local_db = local_db.clone();
        let cancellation_token = cancellation_token.clone();
        let titan_client = titan_client.clone();
        async move {
            trace!("[{UPDATE_TXS_INFO_LOG_PATH}] Loop spawned..");
            'checking_loop: loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        debug!("[{UPDATE_TXS_INFO_LOG_PATH}] Closing [Btc indexer] txs update task, because of cancellation token");
                        break 'checking_loop;
                    },
                    _ = interval.tick() => {
                        let _ = perform_status_update(local_db.clone(), titan_client.clone(), tx_validator.clone())
                            .await
                            .inspect_err(|e|
                                error!("[{UPDATE_TXS_INFO_LOG_PATH}] Error: {}", e)
                            );
                    }
                }
            }
        }
    });

    // Tx finalization tracking task
    tokio::spawn({
        let mut interval = tokio::time::interval(Duration::from_millis(btc_indexer_params.update_interval_millis));
        let local_db = local_db.clone();
        let client = Arc::new(reqwest::Client::new());
        async move {
            trace!("[{FINALIZATION_TRACKING_LOG_PATH}] Loop spawned..");
            'checking_loop: loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        debug!("[{FINALIZATION_TRACKING_LOG_PATH}] Closing [Btc indexer] tx finalization, because of cancellation token");
                        break 'checking_loop;
                    },
                    _ = interval.tick() => {
                        let _ = send_response_to_recipients(client.clone(), local_db.clone())
                            .await
                            .inspect_err(|e|
                                error!("[{FINALIZATION_TRACKING_LOG_PATH}] Error: {}", e)
                            );
                    }
                }
            }
        }
    });
}

/// Gets information about finalized txs and sends already
#[instrument(skip(client, local_db), level = "debug")]
async fn send_response_to_recipients<Db: IndexerDbBounds>(
    client: Arc<reqwest::Client>,
    local_db: Db,
) -> anyhow::Result<()> {
    let updated_txs = local_db.get_values_to_send_response().await?;
    let tasks = spawn_tasks_to_send_response(client, local_db, updated_txs)?;
    let _ = futures::future::join_all(tasks)
        .await
        .into_iter()
        .map(|x| match x {
            Ok(x) => Ok(()),
            Err(e) => {
                bail!(e)
            }
        })
        .collect::<anyhow::Result<Vec<()>>>()?;
    Ok(())
}

#[instrument(skip(client, local_db), level = "debug")]
fn spawn_tasks_to_send_response<Db: IndexerDbBounds>(
    client: Arc<reqwest::Client>,
    local_db: Db,
    txs_to_update_status: Vec<TxTrackingRequestsToSendResponse>,
) -> anyhow::Result<Vec<JoinHandle<()>>> {
    let mut tasks = Vec::with_capacity(txs_to_update_status.len());
    for x in txs_to_update_status {
        tasks.push(tokio::task::spawn({
            let client = client.clone();
            let local_db = local_db.clone();
            async move {
                let resp = BtcIndexerCallbackResponse::Ok {
                    meta: ResponseMeta {
                        outpoint: x.out_point,
                        status: x.review,
                    },
                };
                let client_resp = client.post(x.callback_url.0).json(&resp).send().await;
                match client_resp {
                    Ok(client_resp) => {
                        let status = TrackedReqStatus::Finished;
                        let _ = local_db.finalize_tx_request(x.uuid, status).await.inspect_err(|e| {
                            error!(
                                "[{FINALIZATION_TRACKING_LOG_PATH}] Db finalization error: {}, status: {:?}",
                                e, status
                            )
                        });
                        info!("[{FINALIZATION_TRACKING_LOG_PATH}] Got response: {:?}", client_resp);
                    }
                    Err(e) => {
                        let status = TrackedReqStatus::FailedToSend;
                        let _ = local_db.finalize_tx_request(x.uuid, status).await.inspect_err(|e| {
                            error!(
                                "[{FINALIZATION_TRACKING_LOG_PATH}] Db finalization error: {}, status: {:?}",
                                e, status
                            )
                        });
                        error!("[{FINALIZATION_TRACKING_LOG_PATH}] Error: {}", e);
                    }
                }
            }
        }));
    }
    Ok(tasks)
}

#[instrument(skip(titan_client, local_db, tx_validator), level = "debug")]
async fn perform_status_update<C: TitanApi, Db: IndexerDbBounds, TxValidator: TxArbiterTrait>(
    local_db: Db,
    titan_client: C,
    tx_validator: TxValidator,
) -> anyhow::Result<()> {
    let txs = local_db.get_txs_to_update_status().await?;
    let tasks = spawn_tasks_to_check_txs(txs, local_db, titan_client, tx_validator).await?;
    let _ = futures::future::join_all(tasks)
        .await
        .into_iter()
        .map(|x| match x {
            Ok(x) => Ok(()),
            Err(e) => {
                bail!(e)
            }
        })
        .collect::<anyhow::Result<Vec<()>>>()?;
    Ok(())
}

#[instrument(skip(titan_client, local_db, tx_validator), level = "debug")]
async fn spawn_tasks_to_check_txs<C: TitanApi, Db: IndexerDbBounds, TxValidator: TxArbiterTrait>(
    checked_txs: Vec<TxToUpdateStatus>,
    local_db: Db,
    titan_client: C,
    tx_validator: TxValidator,
) -> anyhow::Result<Vec<JoinHandle<()>>> {
    let mut check_txs_tasks: Vec<JoinHandle<()>> = Vec::with_capacity(checked_txs.len());
    for tx_id in checked_txs {
        let local_db = local_db.clone();
        check_txs_tasks.push({
            let titan_client = titan_client.clone();
            let tx_validator = tx_validator.clone();
            tokio::spawn(async move {
                match titan_client.get_transaction(&tx_id.tx_id.0).await {
                    Ok(tx_to_check) => {
                        let r = check_obtained_transaction(titan_client, tx_validator, &tx_to_check, &tx_id)
                            .await
                            .inspect_err(|e| {
                                error!(
                                    "[{UPDATE_TXS_INFO_LOG_PATH}] Failed to check obtained transaction: {e}, tx_id: {}",
                                    tx_to_check.txid
                                )
                            });
                        if let Ok(res) = r
                            && let TxArbiterResponse::ReviewFormed(review, out_point) = res
                        {
                            let _ = local_db
                                .insert_tx_tracking_report(out_point.clone(), &review, &tx_to_check)
                                .await;
                        }
                    }
                    Err(e) => {
                        error!("[{UPDATE_TXS_INFO_LOG_PATH}] Failed to retrieve account data by address: {e}");
                    }
                };
            })
        });
    }
    Ok(check_txs_tasks)
}

#[instrument(skip(titan_client, tx_validator), level = "debug")]
async fn check_obtained_transaction<C: TitanApi, TxValidator: TxArbiterTrait>(
    titan_client: C,
    tx_validator: TxValidator,
    tx_to_check: &Transaction,
    tx_info: &TxToUpdateStatus,
) -> anyhow::Result<TxArbiterResponse> {
    trace!("[{UPDATE_TXS_INFO_LOG_PATH}] address data successfully received, ");
    Ok(tx_validator.check_tx(titan_client, tx_to_check, tx_info).await?)
}
