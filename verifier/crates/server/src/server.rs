use eyre::{Result, eyre};
use tokio::{self, net::TcpListener, task::JoinHandle};
use verifier_common::config::ServerConfig;

use crate::router::create_router;

pub struct Server {
    config: ServerConfig,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    pub async fn spawn(&self) -> Result<JoinHandle<Result<()>>> {
        let address = self.config.address.clone();

        let app = create_router();

        let listener = TcpListener::bind(address)
            .await
            .map_err(|e| eyre!("Failed to bind to address: {}", e))?;

        let server_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .map_err(|e| eyre!("Failed to serve: {}", e))
        });

        Ok(server_handle)
    }
}
