use axum::{
    Router, debug_handler,
    extract::{Json, State},
    routing::post,
};
use btc_indexer_internals::indexer::BtcIndexer;
use config_parser::config::ServerConfig;
use persistent_storage::init::PersistentRepoShared;
use serde::{Deserialize, Serialize};
use titan_client::TitanClient;
use tracing::info;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    AppState,
    common::{Empty, SocketAddrWrapped},
    error::ServerError,
};

#[derive(Deserialize, Serialize, ToSchema, Debug)]
#[schema(example = json!({
    "spark_address": "sprt1pgss8fxt9jxuv4dgjwrg539s6u06ueausq076xvfej7wdah0htvjlxunt9fa4n",
    "rune_id": "btknrt1p2sy7a8cx5pqfm3u4p2qfqa475fgwj3eg5d03hhk47t66605zf6qg52vj2"
}))]
pub struct TrackTxRequest {
    pub tx_id: String,
    pub callback_url: SocketAddrWrapped,
}

#[derive(Serialize, ToSchema)]
#[schema(example = json!({ "balance": 1000 }))]
pub struct TrackTxResponse {
    pub balance: u128,
}

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
pub(crate) async fn handler(
    State(mut state): State<AppState<TitanClient>>,
    Json(payload): Json<TrackTxRequest>,
) -> Result<Json<Empty>, ServerError> {
    info!("Received track tx: {:?}", payload);
    Ok(Json(Empty {}))
}
