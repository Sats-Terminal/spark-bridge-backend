use std::{
    ffi::{FromVecWithNulError, IntoStringError},
    string::FromUtf8Error,
};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ExecutorError>;

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    CStringIntoStringError(#[from] IntoStringError),
    #[error(transparent)]
    IntoCStringError(#[from] FromVecWithNulError),
    #[error(transparent)]
    FromUtf8Error(#[from] FromUtf8Error),
    #[error("Failed to execute command: {command}, error: {error}")]
    CommandFailure { command: String, error: String },
}
