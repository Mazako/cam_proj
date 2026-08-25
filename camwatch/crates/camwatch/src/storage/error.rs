use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("cannot prepare database directory")]
    CreateDirectory(#[source] std::io::Error),
    #[error("database operation failed")]
    Database(#[source] sqlx::Error),
    #[error("database migration failed")]
    Migration(#[source] sqlx::migrate::MigrateError),
    #[error("database contains an invalid status")]
    InvalidStoredStatus,
}
