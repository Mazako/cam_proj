use std::{path::PathBuf, time::SystemTime};

use super::PortFuture;

pub trait ClipStore: Send + Sync {
    fn store_segment(&self, segment: Segment) -> PortFuture<'_, Result<(), ClipStoreError>>;

    fn create_clip(&self, request: ClipRequest) -> PortFuture<'_, Result<Clip, ClipStoreError>>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    pub camera_id: String,
    pub path: PathBuf,
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipRequest {
    pub event_id: String,
    pub camera_id: String,
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Clip {
    pub event_id: String,
    pub path: PathBuf,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipStoreError {
    NotFound,
    Failed,
}
