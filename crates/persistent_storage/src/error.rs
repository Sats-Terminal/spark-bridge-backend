use config_parser::error::ConfigParserError;
use sqlx::migrate::MigrateError;

#[derive(Debug, thiserror::Error)]
pub enum DbInitError {
    #[error("Unable to retreive env variable for db initialization, error: {0}")]
    UnableToRetrieveEnvVar(String),
    #[error("Failed to establish connection with db, please check url [err: {0}, url: {1}]")]
    FailedToEstablishDbConn(sqlx::Error, String),
    #[error("Failed to initialize initial db config, error: {0}")]
    FailedToParseConfig(#[from] ConfigParserError),
    #[error("Failed to migrate db, error: {0}")]
    FailedToMigrateDb(#[from] MigrateError),
}

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Failed to open Pg client, error: {0}")]
    DbError(#[from] sqlx::Error),
    #[error("Failed to convert type from json, error: {0}")]
    SerdeJsonError(#[from] serde_json::error::Error),
}

pub type Result<T> = std::result::Result<T, DbError>;
