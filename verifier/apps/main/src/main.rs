use global_utils::logger::init_logger;
use log;
use tokio;
use verifier_common::config::VerifierConfig;
use verifier_server::server::Server;

#[tokio::main]
async fn main() {
    let _guard = init_logger();

    let config = VerifierConfig::new(None);

    let server = Server::new(config.server);
    let server_handle = server.spawn().await.unwrap();

    log::info!("Server started");
    let _ = server_handle.await.unwrap();
    log::info!("Server stopped");
}
