use thiserror::Error;

#[derive(Error, Debug)]
pub enum BtcIndexerClientError {
    #[error("Client config type is invalid")]
    InvalidConfigTypeError,
    #[error("Failed to decode hex tx id: {0}")]
    TxIdHexDecodeErr(#[from] bitcoin::hex::HexToArrayError),
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Titan client error: {0}")]
    TitanClientError(#[from] titan_client::Error),
    #[error("vout is out of range. vout: {0}, max_vout: {1}")]
    VoutOutOfRange(u32, u32),
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Decode error: {0}")]
    DecodeError(String),
}
