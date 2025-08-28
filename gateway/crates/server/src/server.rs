use gateway_common::config::ServerConfig;
use crate::router::create_router;
use tokio::{net::TcpListener, task::JoinHandle};
use eyre::{eyre, Result};

pub struct Server {
    config: ServerConfig
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    pub async fn spawn(&self) -> JoinHandle<Result<()>> {
        let address = self.config.address.clone();

        let server_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            let app = create_router();
            
            let listener = TcpListener::bind(address)
                .await
                .map_err(|e| eyre!("Failed to bind to address: {}", e))?;

            axum::serve(listener, app)
                .await
                .map_err(|e| eyre!("Failed to serve: {}", e))
        });

        server_handle
    }
}