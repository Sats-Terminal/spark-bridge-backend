use thiserror::Error;

#[derive(Error, Debug)]
pub enum VerifierClientError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Failed to verify: {0}")]
    VerificationError(String),
    #[error("Deserialize error: {0}")]
    DeserializeError(String),
    #[error("Http error: {0}")]
    HttpError(String),
    #[error("Secp256k1 error: {0}")]
    Secp256k1Error(#[from] bitcoin::secp256k1::Error),
    #[error("Hex error: {0}")]
    HexError(#[from] hex::FromHexError),
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
}
