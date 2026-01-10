use std::io;

pub(crate) type AppResult<T> = Result<T, AppError>;

#[derive(Debug)]
pub(crate) enum AppError {
    ConfigOrHomeNotFound,
    Io(io::Error),
    Picker(nucleo_picker::error::PickError),
    InvalidPath,
    NotFound(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::ConfigOrHomeNotFound => {
                write!(f, "Could not determine home/config directory")
            }
            AppError::Io(e) => write!(f, "{e}"),
            AppError::Picker(e) => write!(f, "{e}"),
            AppError::InvalidPath => write!(f, "Path must be absolute"),
            AppError::NotFound(p) => write!(f, "Not found: {p}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<io::Error> for AppError {
    fn from(e: io::Error) -> Self {
        AppError::Io(e)
    }
}

impl From<nucleo_picker::error::PickError> for AppError {
    fn from(e: nucleo_picker::error::PickError) -> Self {
        AppError::Picker(e)
    }
}
