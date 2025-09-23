use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, extract::State, routing::post};
use axum_test::TestServer;
use btc_indexer_api::api::BtcIndexerCallbackResponse;
use btc_indexer_internals::api::AccountReplenishmentEvent;
use global_utils::common_resp::Empty;
use std::net::TcpListener;
use std::{fmt::Debug, net::SocketAddr, str::FromStr, sync::Arc};
use thiserror::Error;
use titan_client::Transaction;
use tokio::sync::Mutex;
use tracing::{error, info, instrument, warn};
use url::Url;

#[derive(Clone)]
pub struct TestAppState<T: Clone + Send + Sync> {
    pub notifier: Arc<Mutex<Option<tokio::sync::oneshot::Sender<T>>>>,
}

pub const NOTIFY_TX_PATH: &'static str = "/notify_tx";

pub fn obtain_random_localhost_socket_addr() -> anyhow::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let socket_addr = listener.local_addr()?;
    info!(server_addr = ?socket_addr, "Random address:");
    Ok(socket_addr)
}

#[instrument]
pub async fn create_test_notifier_track_tx(
    oneshot_sender: tokio::sync::oneshot::Sender<BtcIndexerCallbackResponse>,
    socket_addr: &SocketAddr,
) -> anyhow::Result<TestServer> {
    let state = TestAppState {
        notifier: Arc::new(Mutex::new(Some(oneshot_sender))),
    };
    let app = Router::new()
        .route(NOTIFY_TX_PATH, post(notify_handler::<BtcIndexerCallbackResponse>))
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
) -> Result<Json<Empty>, MockErr> {
    info!("[callback response received!] Received track tx: {:?}", payload);
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

#[derive(Error, Debug)]
pub enum MockErr {}

impl IntoResponse for MockErr {
    fn into_response(self) -> Response {
        match self {}
    }
}

#[instrument(skip(state))]
async fn _notify_handler_inner<T: Clone + Send + Sync + Debug>(
    mut state: TestAppState<T>,
    payload: T,
) -> Result<Json<Empty>, MockErr> {
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

#[instrument(skip(state))]
async fn notify_tx(
    State(mut state): State<TestAppState<BtcIndexerCallbackResponse>>,
    Json(payload): Json<BtcIndexerCallbackResponse>,
) -> Result<Json<Empty>, MockErr> {
    _notify_handler_inner::<BtcIndexerCallbackResponse>(state, payload).await
}

#[instrument]
pub async fn spawn_notify_server_track_tx(
    socket_addr: SocketAddr,
) -> anyhow::Result<(
    Url,
    tokio::sync::oneshot::Receiver<BtcIndexerCallbackResponse>,
    TestServer,
)> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let test_notifier = create_test_notifier_track_tx(tx, &socket_addr).await?;
    Ok((
        Url::from_str(&format!("http://{}{NOTIFY_TX_PATH}", socket_addr.to_string()))?,
        rx,
        test_notifier,
    ))
}
