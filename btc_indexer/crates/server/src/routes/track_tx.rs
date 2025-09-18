use std::sync::Arc;

use crate::{AppState, error::ServerError};
use axum::extract::{Json, State};
use btc_indexer_api::api::{BtcIndexerCallbackResponse, TrackTxRequest, TrackTxResponse};
use btc_indexer_internals::{api::BtcIndexerApi, indexer::BtcIndexer};
use global_utils::common_resp::Empty;
use global_utils::common_types::{TxIdWrapped, UrlWrapped, get_uuid};
use local_db_store_indexer::{
    PersistentRepoTrait,
    schemas::runes_spark::btc_indexer_work_checkpoint::{BtcIndexerWorkCheckpoint, StatusBtcIndexer, Task, Update},
};
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx::types::{Json as SqlxJson, chrono::Utc};
use titan_client::Transaction;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, trace};
use utoipa::ToSchema;
use uuid::Uuid;

const PATH_TO_LOG: &str = "btc_indexer_server:track_tx";

#[utoipa::path(
    post,
    path = "/track_tx",
    request_body = TrackTxRequest,
    responses(
        (status = 200, description = "Success", body = Empty),
        (status = 400, description = "Bad Request", body = String),
        (status = 500, description = "Internal Server Error", body = String),
    ),
)]
#[instrument(skip(state))]
pub async fn handler<T: titan_client::TitanApi, Db: PersistentRepoTrait + Clone + 'static>(
    State(state): State<AppState<T, Db>>,
    Json(payload): Json<TrackTxRequest>,
) -> Result<TrackTxResponse, ServerError> {
    info!("Received track tx: {:?}", payload);

    //todo: check whether we have spawned task
    // if not spawn
    // add entry to db to check tx

    // todo: thread will invoke indexer_inner to check tx and return some response, after retrieving some response -> send callback response
    let uuid = get_uuid();
    initial_insert(&state, &payload, uuid).await?;

    let cancellation_token = spawn_tx_tracking_task(state.clone(), payload, uuid).await?;
    // {
    //     let mut write_guard = state.task_executor.write().await;
    //     write_guard.insert(uuid, cancellation_token);
    // }
    Ok(Json(Empty {}))
}

async fn initial_insert<T: titan_client::TitanApi, Db: PersistentRepoTrait + Clone + 'static>(
    state: &AppState<T, Db>,
    payload: &TrackTxRequest,
    uuid: Uuid,
) -> Result<(), ServerError> {
    // Insert value in db to save info about processing some value
    let mut conn = state.persistent_storage.get_conn().await?;
    let time_now = Utc::now();
    BtcIndexerWorkCheckpoint {
        uuid,
        status: StatusBtcIndexer::Created,
        task: SqlxJson::from(Task::TrackTx(payload.out_point.tx_id.clone())),
        created_at: time_now,
        callback_url: payload.callback_url.clone(),
        error: None,
        updated_at: time_now,
    }
    .insert(&mut conn)
    .await?;
    Ok(())
}

/// Spawns tracking task for tracking whether we receive event from indexer_internals and send via reqwest msg about completion
#[instrument(skip(app_state))]
pub(crate) async fn spawn_tx_tracking_task<T: titan_client::TitanApi, Db: PersistentRepoTrait + Clone + 'static>(
    app_state: AppState<T, Db>,
    payload: TrackTxRequest,
    uuid: Uuid,
) -> Result<CancellationToken, DbError> {
    let cancellation_token = CancellationToken::new();
    tokio::task::spawn({
        let local_cancellation_token = cancellation_token.child_token();
        async move {
            // let response = _retrieve_tx_info_result(
            //     app_state.persistent_storage,
            //     app_state.btc_indexer,
            //     &payload,
            //     uuid,
            //     local_cancellation_token,
            // )
            // .await;
            // let response = BtcIndexerCallbackResponse::Err {
            //     code: 220,
            //     msg: "helo".into(),
            //     req_meta: payload.clone(),
            // };
            // trace!(
            //     "[{PATH_TO_LOG}] Formed response to send to callback url[{}]: {response:?}",
            //     payload.callback_url.0.to_string()
            // );
            // let _ = app_state
            //     .http_client
            //     .post(payload.callback_url.0.to_string())
            //     .header("Content-Type", "application/json")
            //     .json(&response)
            //     .send()
            //     .await
            //     .inspect_err(|e| error!("[{PATH_TO_LOG}] Receive error on sending response: {:?}", e))
            //     .inspect(|r| debug!("[{PATH_TO_LOG}] (Finishing task execution) Receive response: {r:?}"));
            // app_state.task_executor.write().await.remove(&uuid);
            todo!()
        }
    });
    Ok(cancellation_token)
}

#[instrument(level = "trace", skip(db, indexer, payload), fields(tx_id = payload.out_point.tx_id.0.to_string()) ret)]
async fn _retrieve_tx_info_result<T: titan_client::TitanApi, Db: PersistentRepoTrait + Clone + 'static>(
    db: Db,
    indexer: Arc<BtcIndexer<T, Db>>,
    payload: &TrackTxRequest,
    uuid: Uuid,
    cancellation_token: CancellationToken,
) -> crate::error::Result<Transaction> {
    let confirmed_tx = _inner_retrieve_tx_info_result(indexer, payload, uuid, cancellation_token).await;
    {
        let mut conn = db.get_conn().await?;
        let time_now = Utc::now();
        match confirmed_tx.as_ref() {
            Ok(_) => {
                BtcIndexerWorkCheckpoint::update(
                    &mut conn,
                    &uuid,
                    &Update {
                        status: Some(StatusBtcIndexer::FinishedSuccess),
                        error: None,
                        updated_at: Some(time_now),
                    },
                )
                .await?;
            }
            Err(e) => {
                BtcIndexerWorkCheckpoint::update(
                    &mut conn,
                    &uuid,
                    &Update {
                        status: Some(StatusBtcIndexer::FinishedError),
                        error: Some(e.to_string()),
                        updated_at: Some(time_now),
                    },
                )
                .await?;
            }
        }
    }
    confirmed_tx
}

async fn _inner_retrieve_tx_info_result<T: titan_client::TitanApi, Db: PersistentRepoTrait + Clone + 'static>(
    indexer: Arc<BtcIndexer<T, Db>>,
    payload: &TrackTxRequest,
    uuid: Uuid,
    cancellation_token: CancellationToken,
) -> Result<Transaction, ServerError> {
    // let oneshot_receiver = indexer
    //     .check_tx_changes(payload.tx_id.0, uuid)
    //     .await
    //     .inspect_err(|e| error!("[{PATH_TO_LOG}] Occurred error on signing on tx updates via channel, err: {e}"))?;
    // tokio::select! {
    //     _ = cancellation_token.cancelled() => {
    //         info!("[{PATH_TO_LOG}] Position manager signal listener cancelled");
    //         Err(ServerError::TaskCancelled(PATH_TO_LOG.to_string()))
    //     }
    //     confirmed_tx = oneshot_receiver => {
    //         Ok(confirmed_tx?)
    //     }
    // }
    todo!()
}
