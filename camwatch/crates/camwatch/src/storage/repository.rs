use std::time::SystemTime;

use sqlx::query_as;
use uuid::Uuid;

use super::{
    Camera, Database, Event, EventStatus, NewCamera, NewEvent, NewSegment, NewUpload, Segment,
    StorageError, Upload, UploadStatus, unix_time_millis,
};

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

    pub async fn create_event(&self, event: NewEvent) -> Result<Event, StorageError> {
        let id = Uuid::now_v7().to_string();
        let now = unix_time_millis(SystemTime::now()).unwrap_or_default();
        let status = EventStatus::Recording;

        sqlx::query(
            "INSERT INTO \"events\" (
                id, camera_id, started_at, \"trigger\", status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&event.camera_id)
        .bind(event.started_at)
        .bind(&event.trigger)
        .bind(status)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(Event {
            id,
            camera_id: event.camera_id,
            started_at: event.started_at,
            ended_at: None,
            trigger: event.trigger,
            clip_path: None,
            clip_duration_ms: None,
            status,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_event(&self, id: &str) -> Result<Option<Event>, StorageError> {
        let event = query_as::<_, Event>(
            "SELECT id, camera_id, started_at, ended_at, \"trigger\", clip_path, clip_duration_ms,
                    status, created_at, updated_at
             FROM \"events\" WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(event)
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

    pub async fn create_upload(&self, upload: NewUpload) -> Result<Upload, StorageError> {
        let id = Uuid::now_v7().to_string();
        let now = unix_time_millis(SystemTime::now()).unwrap_or_default();
        let status = UploadStatus::Pending;

        sqlx::query(
            "INSERT INTO uploads (
                id, event_id, provider, status, attempt_count, next_attempt_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, 0, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&upload.event_id)
        .bind(&upload.provider)
        .bind(status)
        .bind(upload.next_attempt_at)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(Upload {
            id,
            event_id: upload.event_id,
            provider: upload.provider,
            status,
            attempt_count: 0,
            next_attempt_at: upload.next_attempt_at,
            remote_file_id: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_upload(&self, id: &str) -> Result<Option<Upload>, StorageError> {
        let upload = query_as::<_, Upload>(
            "SELECT id, event_id, provider, status, attempt_count, next_attempt_at,
                    remote_file_id, last_error, created_at, updated_at
             FROM uploads WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(upload)
    }
}
