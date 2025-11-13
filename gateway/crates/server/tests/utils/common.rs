use std::{
    net::SocketAddr,
    str::FromStr,
    sync::{Arc, LazyLock},
};

use axum::{
    Json, Router, debug_handler,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use axum_test::TestServer;
use frost::signer::FrostSigner;
use global_utils::{
    common_resp::Empty,
    config_path::ConfigPath,
    config_variant::ConfigVariant,
    logger::{LoggerGuard, init_logger},
};
use persistent_storage::{
    config::PostgresDbCredentials,
    init::{PostgresPool, PostgresRepo},
};
use sqlx::migrate::Migrator;
use tokio::net::TcpListener;
use tracing::{info, instrument};
use url::Url;
use verifier_server::init::{AppState, VerifierApi, create_app};

pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());
pub static MIGRATOR: Migrator = sqlx::migrate!("../local_db_store/migrations");

pub const CONFIG_PATH: &str = "../../../infrastructure/configurations/gateway/dev.toml";
pub const PATH_TO_AMAZON_CA: &str = "../../../infrastructure/configurations/certificates/Amazon-Root-CA.pem";
pub const PATH_TO_FLASHNET: &str = "../../../infrastructure/configurations/certificates/Flashnet-CA.pem";

pub fn obtain_random_localhost_socket_addr() -> eyre::Result<SocketAddr> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let socket_addr = listener.local_addr()?;
    info!(server_addr = ?socket_addr, "Random address:");
    Ok(socket_addr)
}
