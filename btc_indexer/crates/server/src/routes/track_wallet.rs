use std::sync::Arc;

use axum::extract::{Json, State};
use btc_indexer_internals::{
    api::{AccountReplenishmentEvent, BtcIndexerApi},
    indexer::BtcIndexer,
};
use global_utils::common_types::UrlWrapped;
use persistent_storage::init::PersistentRepoShared;
use serde::{Deserialize, Serialize};
use titan_client::TitanApi;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, trace};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{AppState, common::Empty, error::ServerError, routes::common::api_result_request::ApiResponseOwned};

const PATH_TO_LOG: &str = "btc_indexer_server:track_wallet";

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[schema(example = json!({
    "wallet": "sprt1pgss8fxt9jxuv4dgjwrg539s6u06ueausq076xvfej7wdah0htvjlxunt9fa4n",
    "callback_url": "127.0.0.1:8080"
}))]
pub struct TrackWalletRequest {
    pub wallet_id: String,
    pub callback_url: UrlWrapped,
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
#[instrument(skip(state))]
pub async fn handler(
    State(state): State<AppState<impl titan_client::TitanApi>>,
    Json(payload): Json<TrackWalletRequest>,
) -> Result<Json<Empty>, ServerError> {
    info!("Received TrackWalletRequest: {:?}", payload);
    //todo: save state of program before handling requests

    let (uuid, cancellation_token) = spawn_wallet_tracking_task(state.clone(), payload);
    {
        let mut write_guard = state.cached_tasks.write().await;
        write_guard.insert(uuid, cancellation_token);
    }
    Ok(Json(Empty {}))
}

/// Spawns tracking task for tracking whether we receive event from indexer_internals and send via reqwest msg about completion
#[instrument(skip(app_state))]
pub(crate) fn spawn_wallet_tracking_task(
    app_state: AppState<impl titan_client::TitanApi>,
    payload: TrackWalletRequest,
) -> (Uuid, CancellationToken) {
    let uuid = Uuid::new_v4();
    let cancellation_token = CancellationToken::new();
    tokio::task::spawn({
        let local_cancellation_token = cancellation_token.child_token();
        async move {
            let response = _retrieve_account_info_result(
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

#[instrument(level = "trace", skip(_db, indexer, payload), fields(tx_id=payload.wallet_id) ret)]
async fn _retrieve_account_info_result(
    _db: PersistentRepoShared,
    indexer: Arc<BtcIndexer<impl TitanApi>>,
    payload: &TrackWalletRequest,
    cancellation_token: CancellationToken,
) -> crate::error::Result<AccountReplenishmentEvent> {
    let oneshot_receiver = indexer.track_account_changes(&payload.wallet_id).inspect_err(|e| {
        //todo: maybe handle error somehow | ?notify about error and retry signing? | ?return error to url?
        error!("Occurred error on signing on tx updates via channel, err: {e}")
    })?;
    tokio::select! {
        _ = cancellation_token.cancelled() => {
            info!("[{PATH_TO_LOG}] Position manager signal listener cancelled");
            Err(ServerError::TaskCancelled(PATH_TO_LOG.to_string()))
        }
        confirmed_wallet_info = oneshot_receiver => {
            Ok(confirmed_wallet_info?)
        }
    }
}
