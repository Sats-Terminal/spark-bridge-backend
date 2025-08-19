use config_parser::error::ConfigParserError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ServerError>;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error(transparent)]
    ActixError(#[from] actix_web::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("Got error in creating config builder, error: {0}")]
    ConfigCreatingError(#[from] ConfigParserError),
}
