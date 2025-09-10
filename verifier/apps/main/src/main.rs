use anyhow::anyhow;
use global_utils::config_path::ConfigPath;
use global_utils::config_variant::ConfigVariant;
use global_utils::env_parser::lookup_ip_addr;
use global_utils::logger::init_logger;
use persistent_storage::{config::PostgresDbCredentials, init::PostgresRepo};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::instrument;
use verifier_config_parser::config::ServerConfig;
use verifier_local_db_store::storage::LocalDbStorage;
use verifier_utils::frost_signer::create_frost_signer;

#[instrument(level = "debug", ret)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenv::dotenv();
    let _logger_guard = init_logger();

    let config_path = ConfigPath::from_env()?;

    let app_config = ServerConfig::init_config(ConfigVariant::OnlyOneFilepath(config_path.path))?;
    tracing::debug!("App config: {:?}", app_config);

    let postgres_creds = PostgresDbCredentials::from_db_url()?;
    let store = LocalDbStorage {
        postgres_repo: PostgresRepo::from_config(postgres_creds).await?,
    };
    let shared_store = Arc::new(store);
    let frost_signer = create_frost_signer(app_config.frost_signer, shared_store.clone(), shared_store.clone());
    let app = verifier_server::init::create_app(frost_signer).await?;

    let addr_to_listen = (lookup_ip_addr(&app_config.server.ip)?, app_config.server.port);
    let listener = TcpListener::bind(addr_to_listen)
        .await
        .map_err(|e| anyhow!("Failed to bind to address: {}", e))?;
    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow!("Failed to serve: {}", e))
}
