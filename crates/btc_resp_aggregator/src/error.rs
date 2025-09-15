use persistent_storage::error::DbError;
use reqwest::StatusCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BtcAggregatorError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Invalid user state: {0}")]
    InvalidUserState(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("HTTP error: {0}")]
    HttpError(String),
    #[error("Failed to send notification to verifier code: {code}, error: {message}")]
    FailedToSendMsgToVerifier { code: u16, message: String },
}

#[derive(Debug, Error)]
pub enum BtcTxCheckerError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Invalid user state: {0}")]
    InvalidUserState(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("HTTP error: {0}")]
    HttpError(String),
    #[error("Failed to parse err: {0}")]
    UrlParseError(url::ParseError),
    #[error("Database error, err: {0}")]
    DatabaseError(#[from] DbError),
    #[error("Database error, err: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Btc indexer response error code: {code}, msg: {message}")]
    IndexerResponseError { code: u16, message: String },
}
