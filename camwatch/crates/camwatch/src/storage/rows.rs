use sqlx::FromRow;

use super::{Camera, Event, EventStatus, StorageError, Upload, UploadStatus};

#[derive(FromRow)]
pub(super) struct CameraRow {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) enabled: bool,
    pub(super) rtsp_url_env: String,
    pub(super) onvif_url: Option<String>,
    pub(super) onvif_credentials_env: Option<String>,
    pub(super) motion_min_area: i64,
    pub(super) yolo_confidence: f64,
    pub(super) created_at: i64,
    pub(super) updated_at: i64,
    pub(super) deleted_at: Option<i64>,
}

impl From<CameraRow> for Camera {
    fn from(row: CameraRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            enabled: row.enabled,
            rtsp_url_env: row.rtsp_url_env,
            onvif_url: row.onvif_url,
            onvif_credentials_env: row.onvif_credentials_env,
            motion_min_area: row.motion_min_area,
            yolo_confidence: row.yolo_confidence,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        }
    }
}

#[derive(FromRow)]
pub(super) struct EventRow {
    pub(super) id: String,
    pub(super) camera_id: String,
    pub(super) started_at: i64,
    pub(super) ended_at: Option<i64>,
    pub(super) trigger: String,
    pub(super) clip_path: Option<String>,
    pub(super) clip_duration_ms: Option<i64>,
    pub(super) status: String,
    pub(super) created_at: i64,
    pub(super) updated_at: i64,
}

impl TryFrom<EventRow> for Event {
    type Error = StorageError;

    fn try_from(row: EventRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            camera_id: row.camera_id,
            started_at: row.started_at,
            ended_at: row.ended_at,
            trigger: row.trigger,
            clip_path: row.clip_path,
            clip_duration_ms: row.clip_duration_ms,
            status: EventStatus::parse(&row.status)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[derive(FromRow)]
pub(super) struct UploadRow {
    pub(super) id: String,
    pub(super) event_id: String,
    pub(super) provider: String,
    pub(super) status: String,
    pub(super) attempt_count: i64,
    pub(super) next_attempt_at: Option<i64>,
    pub(super) remote_file_id: Option<String>,
    pub(super) last_error: Option<String>,
    pub(super) created_at: i64,
    pub(super) updated_at: i64,
}

impl TryFrom<UploadRow> for Upload {
    type Error = StorageError;

    fn try_from(row: UploadRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            event_id: row.event_id,
            provider: row.provider,
            status: UploadStatus::parse(&row.status)?,
            attempt_count: row.attempt_count,
            next_attempt_at: row.next_attempt_at,
            remote_file_id: row.remote_file_id,
            last_error: row.last_error,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}
