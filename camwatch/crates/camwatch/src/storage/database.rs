use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{Camera, NewCamera, NewSegment, Segment, StorageError};
use sqlx::{
    SqlitePool,
    migrate::Migrator,
    query_as, query_builder,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

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

impl Database {
    pub async fn seed_cameras(&self, cameras: &[NewCamera]) -> Result<(), StorageError> {
        let now = unix_time_millis(SystemTime::now()).unwrap_or_default();
        let mut transaction = self.pool.begin().await.map_err(StorageError::Database)?;

        for camera in cameras {
            sqlx::query(
                "INSERT INTO cameras (
                    id, name, enabled, rtsp_url_env, onvif_url, onvif_credentials_env,
                    motion_min_area, yolo_confidence, created_at, updated_at
                ) VALUES (?, ?, 1, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&camera.id)
            .bind(&camera.name)
            .bind(&camera.rtsp_url_env)
            .bind(&camera.onvif_url)
            .bind(&camera.onvif_credentials_env)
            .bind(camera.motion_min_area)
            .bind(camera.yolo_confidence)
            .bind(now)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(StorageError::Database)?;
        }

        transaction.commit().await.map_err(StorageError::Database)
    }

    pub async fn camera_count(&self) -> Result<i64, StorageError> {
        sqlx::query_scalar("SELECT COUNT(*) FROM cameras WHERE deleted_at IS NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(StorageError::Database)
    }

    pub async fn get_camera(&self, id: &str) -> Result<Option<Camera>, StorageError> {
        let camera = query_as::<_, Camera>(
            "SELECT id, name, enabled, rtsp_url_env, onvif_url, onvif_credentials_env,
                    motion_min_area, yolo_confidence, created_at, updated_at, deleted_at
             FROM cameras WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(camera)
    }

    pub async fn upsert_segment(&self, segment: NewSegment) -> Result<Segment, StorageError> {
        let now = unix_time_millis(SystemTime::now()).unwrap_or_default();

        let result = sqlx::query_as::<_, Segment>(
            "INSERT INTO segments (
                path, camera_id, started_at, ended_at, size_bytes, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(path) DO UPDATE SET
                camera_id = excluded.camera_id,
                started_at = excluded.started_at,
                ended_at = excluded.ended_at,
                size_bytes = excluded.size_bytes,
                updated_at = excluded.updated_at
                RETURNING camera_id, path, started_at, ended_at, size_bytes",
        )
        .bind(&segment.path)
        .bind(&segment.camera_id)
        .bind(segment.started_at)
        .bind(segment.ended_at)
        .bind(segment.size_bytes)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(result)
    }

    pub async fn segments_overlapping(
        &self,
        camera_id: &str,
        started_at: i64,
        ended_at: i64,
    ) -> Result<Vec<Segment>, StorageError> {
        let segments = query_as::<_, Segment>(
            "SELECT camera_id, path, started_at, ended_at, size_bytes
             FROM segments
             WHERE camera_id = ? AND started_at <= ? AND ended_at >= ?
             ORDER BY started_at, path",
        )
        .bind(camera_id)
        .bind(ended_at)
        .bind(started_at)
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(segments)
    }

    pub async fn get_segments_finished_before(
        &self,
        before: SystemTime,
    ) -> Result<Vec<Segment>, StorageError> {
        let before_timestamp = unix_time_millis(before).unwrap_or_default();

        let segments = query_as::<_, Segment>(
            "SELECT camera_id, path, started_at, ended_at, size_bytes
             FROM segments
             WHERE ended_at < ?",
        )
        .bind(before_timestamp)
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(segments)
    }

    pub async fn remove_segments(&self, paths: &[String]) -> Result<(), StorageError> {
        if paths.is_empty() {
            return Ok(());
        }

        let mut query = query_builder::QueryBuilder::new("DELETE FROM segments WHERE path IN (");
        {
            let mut separated = query.separated(", ");
            for path in paths {
                separated.push_bind(path);
            }
        }
        query.push(")");

        query
            .build()
            .execute(&self.pool)
            .await
            .map_err(StorageError::Database)?;

        Ok(())
    }
}
