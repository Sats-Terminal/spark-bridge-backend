use env_logger;
use log;
use spark_balance_checker_common::config::Config;
use spark_balance_checker_server::server::create_app;
use tokio;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let _ = env_logger::builder().filter_level(log::LevelFilter::Debug).try_init();

    let config = Config::new(None);
    let app = create_app(&config).await;
    let listener = TcpListener::bind(config.server.address.clone()).await.unwrap();

    log::info!("Listening on {}", config.server.address);
    #[cfg(feature = "swagger")]
    {
        log::info!("Swagger UI available at {}/swagger-ui/", config.server.address);
    }
    axum::serve(listener, app).await.unwrap();
}
