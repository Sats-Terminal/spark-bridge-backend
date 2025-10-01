use crate::AppState;
use crate::error::ServerError;
use axum::Json;
use axum::extract::State;
use btc_indexer_api::api::TrackTxResponse;
use btc_indexer_internals::api::BtcIndexerApi;
use btc_indexer_internals::tx_arbiter::TxArbiterTrait;
use global_utils::common_resp::Empty;
use local_db_store_indexer::init::IndexerDbBounds;
use tracing::{info, instrument};

const PATH_TO_LOG: &str = "btc_indexer_server:track_tx";

#[utoipa::path(
    post,
    path = "/healthcheck",
    request_body = Empty,
    responses(
        (status = 200, description = "Success", body = Empty),
        (status = 400, description = "Bad Request", body = String),
        (status = 500, description = "Internal Server Error", body = String),
    ),
)]
#[instrument(skip(state))]
pub async fn handler<T: titan_client::TitanApi, Db: IndexerDbBounds, TxValidator: TxArbiterTrait>(
    State(state): State<AppState<T, Db, TxValidator>>,
) -> Result<TrackTxResponse, ServerError> {
    info!("[{PATH_TO_LOG}] Received healthcheck request...");
    state.btc_indexer.healthcheck().await?;
    Ok(Json(Empty {}))
}
