use std::{path::PathBuf, time::SystemTime};

use crate::storage::Segment;

use super::segment_lease::SegmentLease;

pub struct ClipJob {
    pub event_id: String,
    pub camera_id: String,
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
    pub path: PathBuf,
    pub segments: Vec<Segment>,
    pub(super) _lease: SegmentLease,
}
