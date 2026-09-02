use super::EventStatus;

#[derive(Clone, Debug, PartialEq, sqlx::FromRow)]
pub struct Event {
    pub id: String,
    pub camera_id: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub trigger: String,
    pub clip_path: Option<String>,
    pub clip_duration_ms: Option<i64>,
    pub status: EventStatus,
    pub created_at: i64,
    pub updated_at: i64,
}
