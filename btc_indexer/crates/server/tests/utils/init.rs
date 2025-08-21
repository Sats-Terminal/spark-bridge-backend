use std::{
    net::{IpAddr, SocketAddr, TcpListener},
    sync::LazyLock,
};

use axum_test::TestServer;
use btc_indexer_internals::indexer::{BtcIndexer, IndexerParams};
use config_parser::config::{BtcRpcCredentials, ConfigVariant, PostgresDbCredentials, ServerConfig};
use global_utils::logger::{LoggerGuard, init_logger};
use persistent_storage::init::PostgresRepo;
use tracing::{info, instrument};

pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());

#[instrument(level = "debug", ret)]
pub async fn init_test_server() -> anyhow::Result<TestServer> {
    let _logger_guard = &*TEST_LOGGER;
    let (btc_creds, postgres_creds, config_variant) = (
        BtcRpcCredentials::new()?,
        PostgresDbCredentials::new()?,
        ConfigVariant::Local,
    );
    let app_config = ServerConfig::init_config(config_variant)?;
    let db_pool = PostgresRepo::from_config(postgres_creds).await?.into_shared();
    let btc_indexer = BtcIndexer::with_api(IndexerParams {
        btc_rpc_creds: btc_creds,
        db_pool: db_pool.clone(),
        btc_indexer_params: app_config.btc_indexer_config,
    })?;
    let app = btc_indexer_server::create_app(db_pool, btc_indexer).await;
    let test_server = TestServer::builder().http_transport().build(app.into_make_service())?;
    tracing::info!("Serving local axum test server on {:?}", test_server.server_address());
    Ok(test_server)
}

pub fn obtain_random_addr() -> anyhow::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let socket_addr = listener.local_addr()?;
    info!(server_addr = ?socket_addr, "Random address:");
    Ok(socket_addr)
}
