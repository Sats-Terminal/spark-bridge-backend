use crate::utils::common::{CONFIG_PATH, obtain_random_localhost_socket_addr};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router, debug_handler};
use axum_test::TestServer;
use frost::signer::FrostSigner;
use global_utils::common_resp::Empty;
use global_utils::config_path::ConfigPath;
use persistent_storage::init::{PostgresPool, PostgresRepo};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;
use url::Url;
use verifier_config_parser::config::ServerConfig;
use verifier_local_db_store::storage::LocalDbStorage;
use verifier_server::init::{VerifierApi, create_app};

pub async fn init_mocked_test_server(pool: PostgresPool) -> anyhow::Result<TestServer> {
    let config_path = ConfigPath {
        path: CONFIG_PATH.to_string(),
    };
    let mut server_config = ServerConfig::init_config(config_path.path);
    tracing::debug!("App config: {:?}", server_config);

    let storage = Arc::new(LocalDbStorage {
        postgres_repo: PostgresRepo { pool },
    });

    let mock_health_app = create_mock_healthcheck_app();
    let addr_to_listen = obtain_random_localhost_socket_addr()?;
    let addr_to_listen_url = Url::from_str(&format!("http://{}", addr_to_listen.to_string()))?;
    info!("Addr to send: {addr_to_listen_url}");
    let listener = TcpListener::bind(addr_to_listen).await?;
    tokio::spawn(async move {
        axum::serve(listener, mock_health_app).await.unwrap();
    });

    let frost_signer = FrostSigner::new(
        server_config.frost_signer.identifier,
        storage.clone(),
        storage.clone(),
        server_config.frost_signer.total_participants,
        server_config.frost_signer.threshold,
    )?;
    server_config.btc_indexer.address = addr_to_listen_url.clone();
    server_config.spark_balance_checker.address = addr_to_listen_url.clone();
    server_config.gateway.address = addr_to_listen_url.clone();
    let app = create_app(
        frost_signer,
        server_config.btc_indexer.clone(),
        server_config.spark_balance_checker.clone(),
        server_config.gateway.clone(),
        storage.clone(),
        server_config.clone(),
    )
    .await;
    let test_server = TestServer::builder().http_transport().build(app.into_make_service())?;
    info!("Serving local axum test server on {:?}", test_server.server_address());
    Ok(test_server)
}

fn create_mock_healthcheck_app() -> Router {
    Router::new().route(VerifierApi::HEALTHCHECK_ENDPOINT, post(handle_healthcheck))
}

#[derive(thiserror::Error, Debug)]
enum DraftResult {}

impl IntoResponse for DraftResult {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, "msg".to_string()).into_response()
    }
}

#[debug_handler]
async fn handle_healthcheck() -> Result<Json<Empty>, DraftResult> {
    Ok((Json(Empty {})))
}
