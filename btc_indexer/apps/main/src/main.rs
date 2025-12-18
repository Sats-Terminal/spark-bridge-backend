use std::sync::Arc;

use btc_indexer::{indexer::Indexer, tx_indexer::TxIndexer};
use btc_indexer_client::client_api::{new_btc_indexer_client, IndexerClient};
use btc_indexer_config::AppConfig;
use btc_indexer_local_db_store::storage::LocalDbStorage;
use btc_indexer_server::init::create_app;
use eyre::Result;
use global_utils::{config_path::ConfigPath, logger::init_logger};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    let _logger_guard = init_logger();

    let _ = dotenvy::dotenv();

    tracing::info!("Starting btc indexer");

    let config_path = ConfigPath::from_env().map_err(|e| eyre::eyre!("Failed to parse config path: {}", e))?;
    let app_config = AppConfig::init_config(config_path.path);
    tracing::debug!("App config: {:?}", app_config);

    let storage = Arc::new(LocalDbStorage::new(app_config.database, app_config.network.network).await?);

    let app = create_app(app_config.network.network, storage.clone()).await;

    let btc_indexer_client = new_btc_indexer_client(app_config.indexer_client);
    let cancellation_token = CancellationToken::new();

    let indexer = Indexer::new(
        app_config.btc_indexer.clone(),
        btc_indexer_client.clone(),
        storage.clone(),
        cancellation_token.clone(),
    );

    tokio::spawn(async move {
        indexer.run().await.unwrap();
    });

    if let IndexerClient::Titan(client) = btc_indexer_client.clone() {
        // Only Titan relies on pre-indexed block txids; Maestro would burn credits and disk space.
        let mut tx_indexer = TxIndexer::new(
            client,
            storage.clone(),
            cancellation_token.clone(),
            app_config.btc_indexer.clone(),
        );

        tokio::spawn(async move {
            tx_indexer.run().await.unwrap();
        });
    } else {
        tracing::info!("Tx indexer disabled for Maestro client");
    }

    tracing::info!("Listening on {:?}", app_config.server.hostname);
    let listener = TcpListener::bind(app_config.server.hostname).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
