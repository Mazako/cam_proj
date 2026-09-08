use std::{path::PathBuf, sync::Arc, time::SystemTime};

use super::Segment;

pub struct ClipJob {
    pub event_id: String,
    pub camera_id: String,
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
    pub path: PathBuf,
    pub segments: Vec<Arc<Segment>>,
}
