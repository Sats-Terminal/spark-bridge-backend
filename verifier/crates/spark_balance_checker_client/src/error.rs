use thiserror::Error;

#[derive(Error, Debug)]
pub enum SparkBalanceCheckerClientError {
    #[error("Http error: {0}")]
    HttpError(String),
    #[error("Deserialize error: {0}")]
    DeserializeError(String),
}
