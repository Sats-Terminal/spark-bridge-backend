use std::sync::Arc;

use crate::{AppState, error::ServerError};
use axum::extract::{Json, State};
use btc_indexer_api::api::{BtcIndexerCallbackResponse, TrackTxRequest, TrackTxResponse};
use btc_indexer_internals::{api::BtcIndexerApi, indexer::BtcIndexer};
use global_utils::common_resp::Empty;
use global_utils::common_types::{TxIdWrapped, UrlWrapped, get_uuid};
use local_db_store_indexer::PersistentRepoTrait;
use local_db_store_indexer::init::IndexerDbBounds;
use local_db_store_indexer::schemas::track_tx_requests_storage::TxRequestsTrackingStorageTrait;
use local_db_store_indexer::schemas::tx_tracking_storage::TxTrackingStorageTrait;
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
pub async fn handler<T: titan_client::TitanApi, Db: IndexerDbBounds>(
    State(state): State<AppState<T, Db>>,
    Json(payload): Json<TrackTxRequest>,
) -> Result<TrackTxResponse, ServerError> {
    info!("Received track tx: {:?}", payload);
    let uuid = get_uuid();
    state.btc_indexer.check_tx_changes(uuid, payload).await?;
    Ok(Json(Empty {}))
}
