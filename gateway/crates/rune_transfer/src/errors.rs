use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuneTransferError {
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Hash error: {0}")]
    HashError(String),
}
