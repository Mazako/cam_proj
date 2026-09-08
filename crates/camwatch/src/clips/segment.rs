use std::{path::PathBuf, time::SystemTime};

#[derive(Debug, PartialEq)]
pub struct Segment {
    pub camera_id: String,
    pub path: PathBuf,
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
    pub size_bytes: u64,
}
