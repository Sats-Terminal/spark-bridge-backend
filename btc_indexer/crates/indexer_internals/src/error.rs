use persistent_storage::error::DatabaseError;

#[derive(Debug, thiserror::Error)]
pub enum BtcIndexerError {
    #[error("Failed to initialize, error: {0}")]
    RpcInitError(#[from] bitcoincore_rpc::Error),
    #[error("Receive titan tcp client, error: {0}")]
    TitanTcpClientError(#[from] titan_client::TitanTcpClientError),
    #[error("Receive db client failure, error: {0}")]
    DatabaseError(#[from] DatabaseError),
}

pub type Result<T> = std::result::Result<T, BtcIndexerError>;
