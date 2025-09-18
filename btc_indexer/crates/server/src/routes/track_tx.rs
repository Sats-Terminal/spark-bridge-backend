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

    let uuid = get_uuid();
    create_checkpoint(&state, &payload, uuid).await?;
    state
        .btc_indexer
        .send_new_tx_to_tracking_thread(payload.btc_address, payload.out_point, payload.amount)
        .await?;
    Ok(Json(Empty {}))
}

async fn create_checkpoint<T: titan_client::TitanApi, Db: PersistentRepoTrait + Clone + 'static>(
    state: &AppState<T, Db>,
    payload: &TrackTxRequest,
    uuid: Uuid,
) -> Result<(), ServerError> {
    // Insert value in db to save info about processing some value
    // Insert value in db to save info about processing some value

    // 1. Insert or get tracked tx (unique by txid+vout)
    // let tracked_tx_id = {
    //     // Pseudocode: adapt to your ORM/SQL
    //     // Try to find existing tracked tx
    //     let txid = payload.out_point.txid.clone();
    //     let vout = payload.out_point.vout;
    //     let tracked = sqlx::query!(
    //         "SELECT id FROM btc_tracked_tx WHERE txid = $1 AND vout = $2",
    //         txid,
    //         vout
    //     )
    //         .fetch_optional(&mut conn)
    //         .await?;
    //
    //     if let Some(row) = tracked {
    //         row.id
    //     } else {
    //         // Insert new tracked tx
    //         let rec = sqlx::query!(
    //             "INSERT INTO btc_tracked_tx (txid, vout, created_at, status) VALUES ($1, $2, $3, $4) RETURNING id",
    //             txid,
    //             vout,
    //             time_now,
    //             "Created"
    //         )
    //             .fetch_one(&mut conn)
    //             .await?;
    //         rec.id
    //     }
    // };

    // 2. Insert new track request referencing tracked_tx_id
    // sqlx::query!(
    //     "INSERT INTO btc_track_tx_request (uuid, tracked_tx_id, callback_url, created_at) VALUES ($1, $2, $3, $4)",
    //
    //     tracked_tx_id,
    //     payload.callback_url,
    //     time_now
    // )
    // .execute(&mut conn)
}
