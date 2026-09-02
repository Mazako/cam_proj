#[derive(Clone, Debug, PartialEq)]
pub struct NewUpload {
    pub event_id: String,
    pub provider: String,
    pub next_attempt_at: Option<i64>,
}
