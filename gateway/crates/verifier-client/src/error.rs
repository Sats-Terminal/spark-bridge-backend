use thiserror::Error;

#[derive(Error, Debug)]
pub enum VerifierClientError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Deserialize error: {0}")]
    DeserializeError(String),
    #[error("Http error: {0}")]
    HttpError(String),
}
