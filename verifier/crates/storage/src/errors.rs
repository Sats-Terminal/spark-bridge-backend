use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database error: {0}")]
    BadRequest(String),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
}
