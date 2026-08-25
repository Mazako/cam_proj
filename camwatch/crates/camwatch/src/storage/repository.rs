use sqlx::query_as;
use uuid::Uuid;

use super::{
    Camera, Database, Event, EventStatus, NewCamera, NewEvent, NewUpload, StorageError, Upload,
    UploadStatus,
    database::unix_time_millis,
    rows::{CameraRow, EventRow, UploadRow},
};

impl Database {
    pub async fn seed_cameras(&self, cameras: &[NewCamera]) -> Result<(), StorageError> {
        let now = unix_time_millis();
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
        let row = query_as::<_, CameraRow>(
            "SELECT id, name, enabled, rtsp_url_env, onvif_url, onvif_credentials_env,
                    motion_min_area, yolo_confidence, created_at, updated_at, deleted_at
             FROM cameras WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        Ok(row.map(Camera::from))
    }

    pub async fn create_event(&self, event: NewEvent) -> Result<Event, StorageError> {
        let id = Uuid::now_v7().to_string();
        let now = unix_time_millis();
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
        .bind(status.as_str())
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
        let row = query_as::<_, EventRow>(
            "SELECT id, camera_id, started_at, ended_at, \"trigger\", clip_path, clip_duration_ms,
                    status, created_at, updated_at
             FROM \"events\" WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        row.map(Event::try_from).transpose()
    }

    pub async fn create_upload(&self, upload: NewUpload) -> Result<Upload, StorageError> {
        let id = Uuid::now_v7().to_string();
        let now = unix_time_millis();
        let status = UploadStatus::Pending;

        sqlx::query(
            "INSERT INTO uploads (
                id, event_id, provider, status, attempt_count, next_attempt_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, 0, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&upload.event_id)
        .bind(&upload.provider)
        .bind(status.as_str())
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
        let row = query_as::<_, UploadRow>(
            "SELECT id, event_id, provider, status, attempt_count, next_attempt_at,
                    remote_file_id, last_error, created_at, updated_at
             FROM uploads WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::Database)?;

        row.map(Upload::try_from).transpose()
    }
}
