use gateway_common::config::GatewayConfig;
use gateway_server::server::Server;
use tokio;
use global_utils::logger::init_logger;
use log;

#[tokio::main]
async fn main() {
    let _guard = init_logger();

    let config = GatewayConfig::new(None);
    let server = Server::new(config.server);
    let server_handle = server.spawn().await.unwrap();

    log::info!("Server started");
    let _ = server_handle.await.unwrap();
    log::info!("Server stopped");
}