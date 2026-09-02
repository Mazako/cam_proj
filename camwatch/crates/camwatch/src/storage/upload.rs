use super::UploadStatus;

#[derive(Clone, Debug, PartialEq, sqlx::FromRow)]
pub struct Upload {
    pub id: String,
    pub event_id: String,
    pub provider: String,
    pub status: UploadStatus,
    pub attempt_count: i64,
    pub next_attempt_at: Option<i64>,
    pub remote_file_id: Option<String>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}
