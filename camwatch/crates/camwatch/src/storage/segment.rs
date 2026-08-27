#[derive(Clone, Debug, PartialEq)]
pub struct NewSegment {
    pub camera_id: String,
    pub path: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub size_bytes: i64,
}

#[derive(Clone, Debug, PartialEq, sqlx::FromRow)]
pub struct Segment {
    pub camera_id: String,
    pub path: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub size_bytes: i64,
}
