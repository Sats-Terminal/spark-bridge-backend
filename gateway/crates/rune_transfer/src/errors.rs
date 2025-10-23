use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuneTransferError {
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Hash error: {0}")]
    HashError(String),
    #[error("URL parse error: {0}")]
    URLParseError(#[from] url::ParseError),
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
}
