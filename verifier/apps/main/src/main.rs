use anyhow::anyhow;
use global_utils::config_variant::ConfigVariant;
use global_utils::env_parser::lookup_ip_addr;
use global_utils::logger::init_logger;
use tokio;
use tokio::net::TcpListener;
use tracing::instrument;
use verifier_config_parser::config::ServerConfig;
use verifier_local_db_store::PostgresDbCredentials;
use verifier_local_db_store::init::LocalDbStorage;

#[instrument(level = "debug", ret)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _logger_guard = init_logger();

    let app_config = ServerConfig::init_config(ConfigVariant::init())?;
    let postgres_creds = PostgresDbCredentials::from_envs()?;
    let db_pool = LocalDbStorage::from_config(postgres_creds).await?;
    let app = verifier_server::init::create_app(db_pool).await?;

    let addr_to_listen = (lookup_ip_addr(&app_config.server.ip)?, app_config.server.port);
    let listener = TcpListener::bind(addr_to_listen)
        .await
        .map_err(|e| anyhow!("Failed to bind to address: {}", e))?;
    Ok(axum::serve(listener, app)
        .await
        .map_err(|e| anyhow!("Failed to serve: {}", e))?)
}
