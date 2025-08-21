use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

use axum::{
    Router,
    extract::{Json, State},
    routing::post,
};
use btc_indexer_internals::indexer::BtcIndexer;
use config_parser::config::ServerConfig;
use persistent_storage::init::PersistentRepoShared;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;
use utoipa::{
    OpenApi, ToSchema, openapi,
    openapi::{Object, SchemaFormat},
};
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    AppState,
    common::{Empty, SocketAddrWrapped},
    error::ServerError,
};

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[schema(example = json!({
    "wallet": "sprt1pgss8fxt9jxuv4dgjwrg539s6u06ueausq076xvfej7wdah0htvjlxunt9fa4n",
    "callback_url": "127.0.0.1:8080"
}))]
pub struct TrackWalletRequest {
    pub wallet: String,
    pub callback_url: SocketAddrWrapped,
}

#[utoipa::path(
    post,
    path = "/track_wallet",
    request_body = TrackWalletRequest,
    responses(
        (status = 200, description = "Success", body = Empty),
        (status = 400, description = "Bad Request", body = String),
        (status = 500, description = "Internal Server Error", body = String),
    ),
)]
pub(crate) async fn handler<C>(
    State(_state): State<AppState<C>>,
    Json(payload): Json<TrackWalletRequest>,
) -> Result<Json<Empty>, ServerError> {
    info!("Received TrackWalletRequest: {:?}", payload);
    Ok(Json(Empty {}))
}
