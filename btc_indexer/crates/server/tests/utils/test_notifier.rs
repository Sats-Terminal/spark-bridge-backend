use std::{fmt::Debug, net::SocketAddr, str::FromStr, sync::Arc};

use axum::{Json, Router, extract::State, routing::post};
use axum_test::TestServer;
use btc_indexer_server::{
    AppState, common::Empty, error::ServerError, routes::common::api_result_request::ApiResponseOwned,
};
use titan_client::Transaction;
use tokio::sync::Mutex;
use tracing::{error, info, instrument, warn};
use url::Url;

#[derive(Clone)]
pub struct TestAppState<T: Clone + Send + Sync> {
    pub notifier: Arc<Mutex<Option<tokio::sync::oneshot::Sender<T>>>>,
}

#[instrument]
pub async fn create_test_notifier_track_tx(
    oneshot_sender: tokio::sync::oneshot::Sender<ApiResponseOwned<Transaction>>,
    socket_addr: &SocketAddr,
) -> anyhow::Result<TestServer> {
    let state = TestAppState {
        notifier: Arc::new(Mutex::new(Some(oneshot_sender))),
    };
    let app = Router::new()
        .route("/notify", post(notify_handler::<ApiResponseOwned<Transaction>>))
        .with_state(state);
    TestServer::builder()
        .http_transport()
        .http_transport_with_ip_port(Some(socket_addr.ip()), Some(socket_addr.port()))
        .build(app.into_make_service())
}

#[instrument(skip(state))]
async fn notify_handler<T: Clone + Send + Sync + Debug>(
    State(mut state): State<TestAppState<T>>,
    Json(payload): Json<T>,
) -> Result<Json<Empty>, ServerError> {
    info!("Received track tx: {:?}", payload);
    //todo: save state of program before handling requests
    let oneshot = state.notifier.lock().await.take();
    if let Some(oneshot_sender) = oneshot {
        info!("Sending notification about response: {payload:?}");
        let _ = oneshot_sender
            .send(payload)
            .inspect_err(|e| error!("Failed to send track tx: {:?}", e));
    } else {
        warn!("No notifier has been set, (trying to send msg: {payload:?}");
    }
    Ok(Json(Empty {}))
}

#[instrument]
pub async fn spawn_notify_server_track_tx(
    socket_addr: SocketAddr,
) -> anyhow::Result<(
    Url,
    tokio::sync::oneshot::Receiver<ApiResponseOwned<Transaction>>,
    TestServer,
)> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let test_notifier = create_test_notifier_track_tx(tx, &socket_addr).await?;
    Ok((
        Url::from_str(&format!("http://{socket_addr}/notify"))?,
        rx,
        test_notifier,
    ))
}
