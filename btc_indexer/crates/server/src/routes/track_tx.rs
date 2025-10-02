use crate::{AppState, error::ServerError};
use axum::extract::{Json, State};
use btc_indexer_api::api::{TrackTxRequest, TrackTxResponse};
use btc_indexer_internals::api::BtcIndexerApi;
use btc_indexer_internals::tx_arbiter::TxArbiterTrait;
use global_utils::common_resp::Empty;
use global_utils::common_types::get_uuid;
use local_db_store_indexer::init::IndexerDbBounds;

use tracing::{info, instrument};

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
pub async fn handler<T: titan_client::TitanApi, Db: IndexerDbBounds, TxValidator: TxArbiterTrait>(
    State(state): State<AppState<T, Db, TxValidator>>,
    Json(payload): Json<TrackTxRequest>,
) -> Result<TrackTxResponse, ServerError> {
    info!("[{PATH_TO_LOG}] Received track tx: {:?}", payload);
    let uuid = get_uuid();
    state.btc_indexer.check_tx_changes(uuid, &payload).await?;
    Ok(Json(Empty {}))
}
