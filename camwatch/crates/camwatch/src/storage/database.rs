use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

use super::StorageError;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct Database {
    pub(super) pool: SqlitePool,
}

impl Database {
    pub async fn open(path: &Path) -> Result<(Self, bool), StorageError> {
        let was_created = !path.exists();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(StorageError::CreateDirectory)?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(StorageError::Database)?;

        MIGRATOR.run(&pool).await.map_err(StorageError::Migration)?;

        Ok((Self { pool }, was_created))
    }
}

pub(crate) fn unix_time_millis(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis()
        .try_into()
        .ok()
}
