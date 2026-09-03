use std::{fmt, net::SocketAddr};

use camwatch::{bucket::R2Error, storage::StorageError};
use thiserror::Error;

#[derive(Debug, PartialEq, Eq)]
pub struct NonLoopbackBindAddress(pub SocketAddr);

impl fmt::Display for NonLoopbackBindAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "server bind address must be 127.0.0.1 or ::1, got {}",
            self.0
        )
    }
}

impl std::error::Error for NonLoopbackBindAddress {}

#[derive(Debug, Error)]
pub enum ServerStartupError {
    #[error("a new database requires at least one camera")]
    EmptyInitialDatabase,
    #[error("database error")]
    Database(#[source] StorageError),
    #[error("R2 configuration error")]
    R2Configuration(#[source] R2Error),
}
