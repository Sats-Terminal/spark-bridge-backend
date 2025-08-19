use env_logger;
use spark_balance_checker_server::server::create_app;
use spark_balance_checker_common::config::Config;
use tokio;
use tokio::net::TcpListener;
use log;

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
