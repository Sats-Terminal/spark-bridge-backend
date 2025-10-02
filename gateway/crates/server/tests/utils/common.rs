use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router, debug_handler};
use axum_test::TestServer;
use frost::signer::FrostSigner;
use global_utils::common_resp::Empty;
use global_utils::config_path::ConfigPath;
use global_utils::config_variant::ConfigVariant;
use global_utils::logger::{LoggerGuard, init_logger};
use persistent_storage::config::PostgresDbCredentials;
use persistent_storage::init::{PostgresPool, PostgresRepo};
use sqlx::migrate::Migrator;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{Arc, LazyLock};
use tokio::net::TcpListener;
use tracing::{info, instrument};
use url::Url;

use verifier_server::init::{AppState, VerifierApi, create_app};

pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());
pub static MIGRATOR: Migrator = sqlx::migrate!("../local_db_store/migrations");

pub const CONFIG_PATH: &str = "../../../infrastructure/configurations/gateway/dev.toml";
pub const PATH_TO_AMAZON_CA: &str = "../../../infrastructure/configurations/certificates/Amazon-Root-CA.pem";
pub const PATH_TO_FLASHNET: &str = "../../../infrastructure/configurations/certificates/Flashnet-CA.pem";

pub fn obtain_random_localhost_socket_addr() -> anyhow::Result<SocketAddr> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let socket_addr = listener.local_addr()?;
    info!(server_addr = ?socket_addr, "Random address:");
    Ok(socket_addr)
}
