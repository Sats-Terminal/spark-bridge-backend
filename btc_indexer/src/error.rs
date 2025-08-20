#[derive(Debug, thiserror::Error)]
pub enum BtcIndexerError {
    #[error("Failed to initialize, error: {0}")]
    RpcInitError(#[from] bitcoincore_rpc::Error),
}

pub type Result<T> = std::result::Result<T, BtcIndexerError>;