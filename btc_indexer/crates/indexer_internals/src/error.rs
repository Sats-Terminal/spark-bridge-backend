use persistent_storage::error::DbError;

#[derive(Debug, thiserror::Error)]
pub enum BtcIndexerError {
    #[error("Receive titan tcp client, error: {0}")]
    TitanTcpClientError(#[from] titan_client::TitanTcpClientError),
    #[error("Receive db client failure, error: {0}")]
    DatabaseError(#[from] DbError),
    #[error("Healthcheck failed, error: {0}")]
    HealthcheckError(String),
    #[error("Bitcoin RPC client error: {0}")]
    BitcoinRpcClientError(#[from] bitcoin_rpc_client::BitcoinRpcClientError),
    #[error("Titan configuration missing for regtest network")]
    MissingTitanConfig,
    #[error("Maestro configuration missing for non-regtest network")]
    MissingMaestroConfig,
    #[error("Maestro client error: {0}")]
    MaestroClientError(String),
}

pub type Result<T> = std::result::Result<T, BtcIndexerError>;
