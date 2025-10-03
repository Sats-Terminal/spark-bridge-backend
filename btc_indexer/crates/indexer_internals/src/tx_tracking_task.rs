use crate::tx_arbiter::{TxArbiterResponse, TxArbiterTrait};

use btc_indexer_api::api::ResponseMeta;
use config_parser::config::BtcIndexerParams;
use local_db_store_indexer::init::IndexerDbBounds;
use local_db_store_indexer::schemas::track_tx_requests_storage::{TrackedReqStatus, TxTrackingRequestsToSendResponse};
use local_db_store_indexer::schemas::tx_tracking_storage::TxToUpdateStatus;

use reqwest::Client;

use std::sync::Arc;
use std::time::Duration;
use titan_client::TitanApi;
use titan_types::Transaction;

use tokio::task::JoinSet;

use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::instrument;

/// Spawns tasks  [1] to track already saved txs in db, [2] to send responses to users when we have finalized tx
#[instrument(
    skip(btc_indexer_params, local_db, cancellation_token, titan_client, tx_validator),
    level = "trace"
)]
pub fn spawn<C: TitanApi, Db: IndexerDbBounds, TxValidator: TxArbiterTrait>(
    cancellation_token: CancellationToken,
    local_db: Db,
    btc_indexer_params: BtcIndexerParams,
    titan_client: Arc<C>,
    tx_validator: Arc<TxValidator>,
    task_tracker: &mut TaskTracker,
) {
    task_tracker.spawn({
        let mut interval = tokio::time::interval(Duration::from_millis(btc_indexer_params.update_interval_millis));
        let local_db = local_db.clone();
        let cancellation_token = cancellation_token.clone();
        let titan_client = titan_client.clone();
        let tx_validator = tx_validator.clone();

        async move {
            'checking_loop: loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        tracing::info!("Closing Btc indexer txs update task, because of cancellation token");
                        break 'checking_loop;
                    },
                    _ = interval.tick() => {
                        let result = perform_status_update(local_db.clone(), titan_client.clone(),tx_validator.clone())
                            .await
                            .inspect_err(|e|
                                tracing::error!("Error while performing status update: {}", e)
                            );
                    }
                }
            }
        }
    });

    task_tracker.spawn({
        let mut interval = tokio::time::interval(Duration::from_millis(btc_indexer_params.update_interval_millis));
        let local_db = local_db.clone();
        let client = Arc::new(Client::new());
        async move {
            'checking_loop: loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        tracing::info!("Closing Btc indexer tx finalization, because of cancellation token");
                        break 'checking_loop;
                    },
                    _ = interval.tick() => {
                        let _ = send_response_to_recipients(client.clone(), local_db.clone())
                            .await
                            .inspect_err(|e|
                                tracing::error!("Error: {}", e)
                            );
                    }
                }
            }
        }
    });
}

/// Gets information about finalized txs and sends already
#[instrument(skip_all, level = "trace")]
async fn send_response_to_recipients<Db: IndexerDbBounds>(
    client: Arc<reqwest::Client>,
    local_db: Db,
) -> anyhow::Result<()> {
    let updated_txs = local_db.get_values_to_send_response().await?;
    tracing::info!("Already received txs to send callback response, txs: {updated_txs:?}");
    let tasks = spawn_tasks_to_send_response(client, local_db, updated_txs)?;
    tasks.join_all().await;
    Ok(())
}

#[instrument(skip_all, level = "trace")]
fn spawn_tasks_to_send_response<Db: IndexerDbBounds>(
    client: Arc<Client>,
    local_db: Db,
    txs_to_update_status: Vec<TxTrackingRequestsToSendResponse>,
) -> anyhow::Result<JoinSet<()>> {
    let mut tasks = JoinSet::default();
    for x in txs_to_update_status {
        tasks.spawn({
            tracing::debug!("Request uuid: {:?}", x.uuid);
            let client = client.clone();
            let local_db = local_db.clone();
            _inner_response_task_spawn(x, client, local_db)
        });
    }
    Ok(tasks)
}

#[instrument(skip(local_db, client), level = "trace")]
fn _inner_response_task_spawn<Db: IndexerDbBounds>(
    data: TxTrackingRequestsToSendResponse,
    client: Arc<Client>,
    local_db: Db,
) -> impl Future<Output = ()> {
    tracing::debug!("Sending response to recipient to url: {}", data.callback_url.0);
    async move {
        let resp = ResponseMeta {
            outpoint: data.out_point,
            status: data.review,
            sats_fee_amount: data.transaction.output[data.out_point.vout as usize].value,
        };
        let client_resp = client.post(data.callback_url.0).json(&resp).send().await;
        tracing::debug!("Client response: {:?}", client_resp);
        match client_resp {
            Ok(client_resp) => {
                let status = TrackedReqStatus::Finished;
                let _ = local_db
                    .finalize_tx_request(data.uuid, status)
                    .await
                    .inspect_err(|e| tracing::error!("Db finalization error: {}, status: {:?}", e, status));
                tracing::debug!("Got response: {:?}", client_resp);
            }
            Err(e) => {
                let status = TrackedReqStatus::FailedToSend;
                let _ = local_db
                    .finalize_tx_request(data.uuid, status)
                    .await
                    .inspect_err(|e| tracing::error!("Db finalization error: {}, status: {:?}", e, status));
                tracing::error!("Error: {}", e);
            }
        }
    }
}

#[instrument(skip_all, level = "trace", ret)]
async fn perform_status_update<C: TitanApi, Db: IndexerDbBounds, TxValidator: TxArbiterTrait>(
    local_db: Db,
    titan_client: Arc<C>,
    tx_validator: Arc<TxValidator>,
) -> anyhow::Result<()> {
    let txs = local_db.get_txs_to_update_status().await?;
    tracing::debug!("Performing update for txs: {:?}", txs);
    let tasks = spawn_tasks_to_check_txs(txs, local_db, titan_client, tx_validator).await?;
    tasks.join_all().await;
    Ok(())
}

#[instrument(skip_all, level = "trace")]
async fn spawn_tasks_to_check_txs<C: TitanApi, Db: IndexerDbBounds, TxValidator: TxArbiterTrait>(
    checked_txs: Vec<TxToUpdateStatus>,
    local_db: Db,
    titan_client: Arc<C>,
    tx_validator: Arc<TxValidator>,
) -> anyhow::Result<JoinSet<()>> {
    let mut check_txs_tasks = JoinSet::default();
    for tx_id in checked_txs {
        let local_db = local_db.clone();
        check_txs_tasks.spawn({
            let titan_client = titan_client.clone();
            let tx_validator = tx_validator.clone();
            _inner_update_task_spawn(tx_id, local_db, titan_client, tx_validator)
        });
    }
    Ok(check_txs_tasks)
}

#[instrument(skip(titan_client, local_db, tx_validator), level = "trace")]
fn _inner_update_task_spawn<C: TitanApi, Db: IndexerDbBounds, TxValidator: TxArbiterTrait>(
    tx_id: TxToUpdateStatus,
    local_db: Db,
    titan_client: Arc<C>,
    tx_validator: Arc<TxValidator>,
) -> impl Future<Output = ()> {
    async move {
        tracing::debug!("Starting task with tx_id: {:?}", tx_id);
        match titan_client.get_transaction(&tx_id.tx_id.0).await {
            Ok(tx_to_check) => {
                tracing::debug!("Get transaction info: {:?}", tx_to_check);
                let r = check_obtained_transaction(titan_client, tx_validator, &tx_to_check, &tx_id)
                    .await
                    .inspect_err(|e| {
                        tracing::error!("Failed to check obtained transaction: {e}, tx_id: {}", tx_to_check.txid)
                    });
                tracing::debug!("Review finihsed: {:?}", r);
                if let Ok(res) = r
                    && let TxArbiterResponse::ReviewFormed(review, out_point) = res
                {
                    tracing::debug!("Review for txid formed: {:?}", review);
                    let _ = local_db
                        .insert_tx_tracking_report(out_point, &review, &tx_to_check)
                        .await;
                }
            }
            Err(e) => {
                tracing::error!("Failed to retrieve account data by address: {e}");
            }
        };
    }
}

#[instrument(skip_all, level = "trace", ret)]
async fn check_obtained_transaction<C: TitanApi, TxValidator: TxArbiterTrait>(
    titan_client: Arc<C>,
    tx_validator: Arc<TxValidator>,
    tx_to_check: &Transaction,
    tx_info: &TxToUpdateStatus,
) -> anyhow::Result<TxArbiterResponse> {
    tracing::debug!("Checking obtained transaction: {:?}", tx_info);
    Ok(tx_validator.check_tx(titan_client, tx_to_check, tx_info).await?)
}
