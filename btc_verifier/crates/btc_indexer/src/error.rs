#[derive(Debug, thiserror::Error)]
pub enum BtcIndexerError {
    #[error("Failed to initialize, error: {0}")]
    RpcInitError(#[from] bitcoincore_rpc::Error),
    #[error("Receive titan tcp client, error: {0}")]
    TitanTcpClientError(#[from] titan_client::TitanTcpClientError),
}

pub type Result<T> = std::result::Result<T, BtcIndexerError>;
