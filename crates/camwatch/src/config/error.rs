use super::{SecretError, app::AppValidationError, camera::CameraValidationError};
use std::fmt;
use thiserror::Error;

#[derive(Debug, PartialEq, Eq)]
pub struct ValidationErrors<E>(pub Vec<E>);

impl<E> From<Vec<E>> for ValidationErrors<E> {
    fn from(errors: Vec<E>) -> Self {
        Self(errors)
    }
}

impl<E: fmt::Display> fmt::Display for ValidationErrors<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str("; ")?;
            }
            error.fmt(formatter)?;
        }
        Ok(())
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for ValidationErrors<E> {}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read file {path}: {source}")]
    Read {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("invalid configuration")]
    InvalidToml,
    #[error("invalid configuration: {0}")]
    App(#[source] ValidationErrors<AppValidationError>),
    #[error("invalid configuration: {0}")]
    Camera(#[source] ValidationErrors<CameraValidationError>),
    #[error("cannot decrypt configuration")]
    Secrets(#[source] SecretError),
    #[error("invalid configuration: camera IDs must be unique")]
    DuplicateCameraIds,
}

impl From<SecretError> for ConfigError {
    fn from(error: SecretError) -> Self {
        Self::Secrets(error)
    }
}

impl From<Vec<AppValidationError>> for ConfigError {
    fn from(errors: Vec<AppValidationError>) -> Self {
        Self::App(errors.into())
    }
}

impl From<Vec<CameraValidationError>> for ConfigError {
    fn from(errors: Vec<CameraValidationError>) -> Self {
        Self::Camera(errors.into())
    }
}
