use std::{str::FromStr, sync::Arc};

use axum::{
    Router, debug_handler,
    extract::{Json, State},
    response::IntoResponse,
    routing::post,
};
use bitcoin::Txid;
use btc_indexer_internals::{api::BtcIndexerApi, error::BtcIndexerError, indexer::BtcIndexer};
use config_parser::config::ServerConfig;
use persistent_storage::init::{PersistentRepoShared, PersistentRepoTrait};
use reqwest::{Body, Client, Request, Response};
use serde::{Deserialize, Serialize};
use titan_client::{TitanApi, TitanClient, Transaction};
use tokio::sync::oneshot::{Receiver, error::RecvError};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, trace};
use url::Url;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;

use crate::{
    AppState,
    common::{Empty, SocketAddrWrapped, TxIdWrapped, UrlWrapped},
    error::ServerError,
    routes::common::api_result_request::{ApiResponse, ApiResponseOwned},
};

const PATH_TO_LOG: &str = "btc_indexer_server:track_tx";

#[derive(Deserialize, Serialize, ToSchema, Debug)]
#[schema(example = json!({
    "tx_id": "fb0c9ab881331ec7acdd85d79e3197dcaf3f95055af1703aeee87e0d853e81ec",
    "callback_url": "http://127.0.0.1:8080"
}))]
pub struct TrackTxRequest {
    pub tx_id: TxIdWrapped,
    pub callback_url: UrlWrapped,
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
#[instrument(skip(state))]
pub async fn handler(
    State(mut state): State<AppState<impl titan_client::TitanApi>>,
    Json(payload): Json<TrackTxRequest>,
) -> Result<Json<Empty>, ServerError> {
    info!("Received track tx: {:?}", payload);
    //todo: save state of program before handling requests

    let (uuid, cancellation_token) = spawn_tx_tracking_task(state.clone(), payload);
    {
        let mut write_guard = state.cached_tasks.write().await;
        write_guard.insert(uuid, cancellation_token);
    }
    Ok(Json(Empty {}))
}

/// Spawns tracking task for tracking whether we receive event from indexer_internals and send via reqwest msg about completion
#[instrument(skip(app_state))]
pub(crate) fn spawn_tx_tracking_task(
    app_state: AppState<impl titan_client::TitanApi>,
    payload: TrackTxRequest,
) -> (Uuid, CancellationToken) {
    let uuid = Uuid::new_v4();
    let cancellation_token = CancellationToken::new();
    tokio::task::spawn({
        let local_cancellation_token = cancellation_token.child_token();
        async move {
            let response = _retrieve_tx_info_result(
                app_state.persistent_storage,
                app_state.btc_indexer,
                &payload,
                local_cancellation_token,
            )
            .await;
            let response = ApiResponseOwned::from(response).encode_string_json();
            trace!(
                "[{PATH_TO_LOG}] Formed response to send to callback url[{}]: {response:?}",
                payload.callback_url.0.to_string()
            );
            let _ = app_state
                .http_client
                .post(payload.callback_url.0.to_string())
                .header("Content-Type", "application/json")
                .body(response)
                .send()
                .await
                .inspect_err(|e| error!("[{PATH_TO_LOG}] Receive error on sending response: {:?}", e))
                .inspect(|r| debug!("[{PATH_TO_LOG}] (Finishing task execution) Receive response: {r:?}"));
            //todo: update query in db | mark as resolved
            app_state.cached_tasks.write().await.remove(&uuid);
        }
    });
    (uuid, cancellation_token)
}

#[instrument(level = "trace", skip(db, indexer, payload), fields(tx_id=payload.tx_id.0.to_string()) ret)]
async fn _retrieve_tx_info_result(
    db: PersistentRepoShared,
    indexer: Arc<BtcIndexer<impl TitanApi>>,
    payload: &TrackTxRequest,
    cancellation_token: CancellationToken,
) -> crate::error::Result<Transaction> {
    let oneshot_receiver = indexer.track_tx_changes(payload.tx_id.0).inspect_err(|e| {
        //todo: maybe handle error somehow | ?notify about error and retry signing? | ?return error to url?
        error!("Occurred error on signing on tx updates via channel, err: {e}")
    })?;
    tokio::select! {
        _ = cancellation_token.cancelled() => {
            info!("[{PATH_TO_LOG}] Position manager signal listener cancelled");
            Err(ServerError::TaskCancelled(PATH_TO_LOG.to_string()))
        }
        maybe_confirmed_tx = oneshot_receiver => {
            Ok(maybe_confirmed_tx?)
        }
    }
}
