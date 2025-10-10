use tokio;
use eyre::Result;
use btc_indexer_server::init::create_app;
use btc_indexer_config::AppConfig;
use global_utils::config_path::ConfigPath;
use global_utils::logger::init_logger;
use btc_indexer_local_db_store::LocalDbStorage;
use tokio::net::TcpListener;
use axum;
use std::sync::Arc;
use btc_indexer::indexer::Indexer;
use btc_indexer_client::clients::titan::TitanClient;
use btc_indexer_client::client_api::BtcIndexerClientApi;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    init_logger();

    let config_path = ConfigPath::from_env().map_err(|e| eyre::eyre!("Failed to parse config path: {}", e))?;
    let app_config = AppConfig::init_config(config_path.path);
    tracing::debug!("App config: {:?}", app_config);
    
    let storage = Arc::new(LocalDbStorage::new(app_config.database, app_config.network.network).await?);

    let app = create_app(app_config.network.network, storage.clone()).await;

    let titan_client = TitanClient::new(app_config.indexer_client.clone());
    let cancellation_token = CancellationToken::new();

    let indexer = Indexer::new(
        app_config.btc_indexer.clone(),
        titan_client,
        storage.clone(),
        cancellation_token.clone(),
    );

    tokio::spawn(async move {
        indexer.run().await.unwrap();
    });

    let listener = TcpListener::bind(app_config.server.hostname).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
