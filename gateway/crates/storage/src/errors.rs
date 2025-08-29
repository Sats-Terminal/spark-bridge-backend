use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database error: {0}")]
    BadRequest(String),
    #[error("Not found: {0}")]
    NotFound(String),
}