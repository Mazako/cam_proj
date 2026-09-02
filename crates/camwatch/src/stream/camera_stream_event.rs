use std::{path::PathBuf, time::SystemTime};

use super::{CameraStreamStatus, Frame};

#[derive(Clone, Debug, PartialEq)]
pub enum CameraStreamEvent {
    Status(CameraStreamStatus),
    Frame(Frame),
    SegmentFinalized {
        path: PathBuf,
        started_at: SystemTime,
        ended_at: SystemTime,
    },
}
