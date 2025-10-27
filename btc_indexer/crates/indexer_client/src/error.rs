use thiserror::Error;

#[derive(Error, Debug)]
pub enum BtcIndexerClientError {
    #[cfg(feature = "titan-client")]
    #[error("Titan client error: {0}")]
    TitanClientError(#[from] titan_client::Error),
    #[error("vout is out of range. vout: {0}, max_vout: {1}")]
    VoutOutOfRange(u32, u32),
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Decode error: {0}")]
    DecodeError(String),
}
