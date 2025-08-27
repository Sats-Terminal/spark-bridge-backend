use std::{net::IpAddr, str::FromStr};

use btc_indexer_internals::indexer::{BtcIndexer, IndexerParams};
use config_parser::config::{BtcRpcCredentials, ConfigVariant, ServerConfig};
use global_utils::logger::init_logger;
use persistent_storage::{config::PostgresDbCredentials, init::PostgresRepo};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    let _logger_guard = init_logger();
    let (btc_creds, postgres_creds, config_variant) = (
        BtcRpcCredentials::new()?,
        PostgresDbCredentials::new()?,
        ConfigVariant::init(),
    );
    let app_config = ServerConfig::init_config(config_variant)?;
    let db_pool = PostgresRepo::from_config(postgres_creds).await?.into_shared();
    let btc_indexer = BtcIndexer::with_api(IndexerParams {
        btc_rpc_creds: btc_creds,
        db_pool: db_pool.clone(),
        btc_indexer_params: app_config.btc_indexer_config,
    })?;
    let app = btc_indexer_server::create_app(db_pool, btc_indexer).await;

    let addr_to_listen = (
        IpAddr::from_str(&app_config.app_config.http_server_ip)?,
        app_config.app_config.http_server_port,
    );
    let listener = TcpListener::bind(addr_to_listen).await?;

    tracing::info!("Listening on {:?}", addr_to_listen);
    #[cfg(feature = "swagger")]
    tracing::info!("Swagger UI available at {:?}/swagger-ui/", addr_to_listen);

    axum::serve(listener, app).await.unwrap();
    Ok(())
}
