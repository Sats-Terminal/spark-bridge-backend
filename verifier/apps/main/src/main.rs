use std::sync::Arc;

use eyre::Result;
use frost::signer::FrostSigner;
use global_utils::{config_path::ConfigPath, logger::init_logger};
use persistent_storage::{config::PostgresDbCredentials, init::PostgresRepo};
use tokio::net::TcpListener;
use tracing::instrument;
use verifier_config_parser::config::ServerConfig;
use verifier_local_db_store::storage::LocalDbStorage;

#[instrument(level = "debug", ret)]
#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let _logger_guard = init_logger();

    // Create Config
    let config_path = ConfigPath::from_env().map_err(|e| eyre::eyre!("Failed to parse config path: {}", e))?;
    let server_config = ServerConfig::init_config(config_path.path);
    tracing::debug!("App config: {:?}", server_config);

    // Create DB Pool
    let postgres_creds = PostgresDbCredentials {
        url: server_config.database.url.clone(),
    };
    let storage = Arc::new(LocalDbStorage {
        postgres_repo: PostgresRepo::from_config(postgres_creds)
            .await
            .map_err(|e| eyre::eyre!("Failed to create DB pool: {}", e))?,
    });

    // Create Frost Signer
    let frost_signer = FrostSigner::new(
        server_config.frost_signer.identifier,
        storage.clone(),
        storage.clone(),
        server_config.frost_signer.total_participants,
        server_config.frost_signer.threshold,
    )
    .map_err(|e| eyre::eyre!("Failed to create frost signer: {}", e))?;

    // Create App
    let app = verifier_server::init::create_app(
        frost_signer,
        server_config.btc_indexer.clone(),
        server_config.spark_balance_checker.clone(),
        server_config.gateway.clone(),
        storage.clone(),
        server_config.clone(),
    )
    .await;

    // Run App
    let addr_to_listen = format!("{}:{}", server_config.server.ip, server_config.server.port);
    let listener = TcpListener::bind(addr_to_listen)
        .await
        .map_err(|e| eyre::eyre!("Failed to bind listener: {}", e))?;
    axum::serve(listener, app)
        .await
        .map_err(|e| eyre::eyre!("Failed to serve app: {}", e))?;

    Ok(())
}
