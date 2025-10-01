use std::{
    net::{IpAddr, SocketAddr, TcpListener},
    str::FromStr,
    sync::LazyLock,
};

use axum_test::TestServer;
use btc_indexer_internals::indexer::{BtcIndexer, IndexerParams};
use config_parser::config::{BtcRpcCredentials, ServerConfig};
use global_utils::config_variant::ConfigVariant;
use global_utils::logger::{LoggerGuard, init_logger};
use local_db_store_indexer::{PostgresDbCredentials, init::LocalDbStorage};
use tracing::{info, instrument};
use url::Url;

pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../local_db_store/migrations");

/// Init test server with real docker
#[instrument(level = "debug", ret)]
pub async fn init_test_server() -> anyhow::Result<TestServer> {
    let _logger_guard = &*TEST_LOGGER;
    let (btc_creds, postgres_creds, config_variant) = (
        BtcRpcCredentials::new()?,
        PostgresDbCredentials::from_envs()?,
        ConfigVariant::Local,
    );
    let app_config = ServerConfig::init_config(config_variant)?;
    let db_pool = LocalDbStorage::from_config(postgres_creds).await?;
    let btc_indexer = BtcIndexer::with_api(IndexerParams {
        titan_config: app_config.titan_config,
        btc_rpc_creds: btc_creds,
        db_pool: db_pool.clone(),
        btc_indexer_params: app_config.btc_indexer_config,
    })?;
    let app = btc_indexer_server::create_app(db_pool, btc_indexer).await;
    let test_server = TestServer::builder().http_transport().build(app.into_make_service())?;
    tracing::info!("Serving local axum test server on {:?}", test_server.server_address());
    Ok(test_server)
}

pub fn obtain_random_localhost_socket_addr() -> anyhow::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let socket_addr = listener.local_addr()?;
    info!(server_addr = ?socket_addr, "Random address:");
    Ok(socket_addr)
}

pub fn obtain_random_localhost_url() -> anyhow::Result<Url> {
    Ok(Url::from_str(
        &format!("http://{}", obtain_random_localhost_socket_addr()?).to_string(),
    )?)
}
