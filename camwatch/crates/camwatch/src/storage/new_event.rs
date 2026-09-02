#[derive(Clone, Debug, PartialEq)]
pub struct NewEvent {
    pub camera_id: String,
    pub started_at: i64,
    pub trigger: String,
}
