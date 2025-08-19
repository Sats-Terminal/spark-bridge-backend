use env_logger;
use spark_indexer::config::Config;
use spark_indexer::server::create_app;
use tokio;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let config = Config::new(None).unwrap();
    let app = create_app(&config).await;
    let listener = TcpListener::bind(config.server.address.clone())
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
    log::info!("Listening on {}", config.server.address);
}
