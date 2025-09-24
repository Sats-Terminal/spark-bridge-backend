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
use verifier_btc_indexer_client::client::BtcIndexerClient;
use verifier_config_parser::config::{BtcIndexerConfig, GatewayConfig, ServerConfig, SparkBalanceCheckerConfig};
use verifier_gateway_client::client::GatewayClient;
use verifier_local_db_store::storage::LocalDbStorage;
use verifier_server::init::{AppState, VerifierApi, create_app};
use verifier_spark_balance_checker_client::client::SparkBalanceCheckerClient;

pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());
pub static MIGRATOR: Migrator = sqlx::migrate!("../local_db_store/migrations");

pub const CONFIG_PATH: &str = "../../../infrastructure/configurations/verifier_1/dev.toml";
pub const CERT_PATH: &str = "../../../infrastructure/configurations/common/ca.pem";

pub fn obtain_random_localhost_socket_addr() -> anyhow::Result<SocketAddr> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let socket_addr = listener.local_addr()?;
    info!(server_addr = ?socket_addr, "Random address:");
    Ok(socket_addr)
}
