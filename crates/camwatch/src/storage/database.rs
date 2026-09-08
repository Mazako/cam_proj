use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{Camera, NewCamera, StorageError};
use sqlx::{
    SqlitePool,
    migrate::Migrator,
    query_as,
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
    pub async fn list_cameras(&self) -> Result<Vec<Camera>, StorageError> {
        query_as::<_, Camera>(
            "SELECT id, name, enabled, rtsp_url, onvif_url,
                    onvif_credentials, motion_min_area, yolo_confidence,
                    clip_after_motion, created_at, updated_at, deleted_at
             FROM cameras
             WHERE deleted_at IS NULL AND enabled = 1
             ORDER BY name, id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::Database)
    }

    pub async fn upsert_cameras(&self, cameras: &[NewCamera]) -> Result<(), StorageError> {
        let now = unix_time_millis(SystemTime::now()).unwrap_or_default();
        let mut transaction = self.pool.begin().await.map_err(StorageError::Database)?;

        for camera in cameras {
            sqlx::query(
                "INSERT INTO cameras (
                    id, name, enabled, rtsp_url, onvif_url,
                    onvif_credentials, motion_min_area, yolo_confidence,
                    clip_after_motion, created_at, updated_at
                ) VALUES (?, ?, 1, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    enabled = excluded.enabled,
                    rtsp_url = excluded.rtsp_url,
                    onvif_url = excluded.onvif_url,
                    onvif_credentials = excluded.onvif_credentials,
                    motion_min_area = excluded.motion_min_area,
                    yolo_confidence = excluded.yolo_confidence,
                    clip_after_motion = excluded.clip_after_motion,
                    updated_at = excluded.updated_at,
                    deleted_at = NULL",
            )
            .bind(&camera.id)
            .bind(&camera.name)
            .bind(&camera.rtsp_url)
            .bind(&camera.onvif_url)
            .bind(&camera.onvif_credentials)
            .bind(camera.motion_min_area)
            .bind(camera.yolo_confidence)
            .bind(camera.clip_after_motion)
            .bind(now)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(StorageError::Database)?;
        }

        transaction.commit().await.map_err(StorageError::Database)
    }

    pub async fn upsert_camera(&self, camera: &NewCamera) -> Result<(), StorageError> {
        self.upsert_cameras(std::slice::from_ref(camera)).await
    }

    pub async fn camera_count(&self) -> Result<i64, StorageError> {
        sqlx::query_scalar("SELECT COUNT(*) FROM cameras WHERE deleted_at IS NULL AND enabled = 1")
            .fetch_one(&self.pool)
            .await
            .map_err(StorageError::Database)
    }

    pub async fn get_camera(&self, id: &str) -> Result<Option<Camera>, StorageError> {
        let camera = query_as::<_, Camera>(
            "SELECT id, name, enabled, rtsp_url, onvif_url,
                    onvif_credentials, motion_min_area, yolo_confidence,
                    clip_after_motion, created_at, updated_at, deleted_at
             FROM cameras WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(camera)
    }

    pub async fn soft_delete_camera(&self, id: &str) -> Result<bool, StorageError> {
        let now = unix_time_millis(SystemTime::now()).unwrap_or_default();
        let result = sqlx::query(
            "UPDATE cameras
             SET enabled = 0, deleted_at = ?, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL AND enabled = 1",
        )
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(result.rows_affected() == 1)
    }
}
