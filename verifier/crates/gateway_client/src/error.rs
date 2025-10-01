use thiserror::Error;

#[derive(Error, Debug)]
pub enum GatewayClientError {
    #[error("Deserialize error: {0}")]
    DeserializeError(String),
    #[error("Http error: {0}")]
    HttpError(String),
}
