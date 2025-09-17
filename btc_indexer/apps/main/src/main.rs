use btc_indexer_internals::indexer::{BtcIndexer, IndexerParams};
use config_parser::config::{BtcRpcCredentials, ServerConfig};
use global_utils::config_variant::ConfigVariant;
use global_utils::{env_parser::lookup_ip_addr, logger::init_logger};
use local_db_store_indexer::{PostgresDbCredentials, init::LocalDbIndexer};
use tokio::net::TcpListener;
use std::net::SocketAddr;
use std::str::FromStr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenv::dotenv();
    let _logger_guard = init_logger();
    let config_path = std::env::var("CONFIG_PATH").unwrap();
    let app_config = ServerConfig::init_config(ConfigVariant::OnlyOneFilepath(config_path))?;
    let btc_creds = BtcRpcCredentials {
        url: app_config.titan_config.url.clone(),
        network: app_config.network_config.network,
        name: "postgres".to_string(),
        password: "postgres".to_string(),
    };
    let postgres_creds = PostgresDbCredentials {
        url: app_config.database_config.url,
    };
    tracing::info!("one");
    let db_pool = LocalDbIndexer::from_config(postgres_creds).await?;
    tracing::info!("two");
    let btc_indexer = BtcIndexer::with_api(IndexerParams {
        btc_rpc_creds: btc_creds,
        db_pool: db_pool.clone(),
        btc_indexer_params: app_config.btc_indexer_config,
    })?;
    tracing::info!("three");
    let app = btc_indexer_server::create_app(db_pool, btc_indexer).await;
    tracing::info!("four");

    let addr_to_listen = format!("{}:{}", app_config.app_config.http_server_ip, app_config.app_config.http_server_port);
    let listener = TcpListener::bind(addr_to_listen.clone()).await?;

    tracing::info!("Listening on {:?}", addr_to_listen);
    #[cfg(feature = "swagger")]
    tracing::info!("Swagger UI available at {:?}/swagger-ui/", addr_to_listen);

    axum::serve(listener, app).await?;

    Ok(())
}
