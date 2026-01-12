use std::io;
use thiserror::Error;

pub(crate) type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub(crate) enum AppError {
    #[error("Could not determine data directory")]
    DataDirectoryNotFound,

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Picker(#[from] nucleo_picker::error::PickError),

    #[error("Path must be absolute")]
    InvalidPath,

    #[error("Not found: {0}")]
    NotFound(String),
}
