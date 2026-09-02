use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

use crate::storage::{Segment, unix_time_millis};

use super::{ClipJob, segment_lease::SegmentLease};

pub(super) struct ActiveClip {
    camera_id: String,
    started_at: SystemTime,
    ended_at: SystemTime,
    path: PathBuf,
    segments: Vec<Segment>,
    lease: SegmentLease,
}

impl ActiveClip {
    pub(super) fn new(
        camera_id: String,
        detected_at: SystemTime,
        pre_duration: Duration,
        post_duration: Duration,
        path: PathBuf,
        lease: SegmentLease,
    ) -> Self {
        let started_at = detected_at.checked_sub(pre_duration).unwrap();
        let ended_at = detected_at.checked_add(post_duration).unwrap();

        Self {
            camera_id,
            started_at,
            ended_at,
            path,
            segments: Vec::new(),
            lease,
        }
    }

    pub(super) fn add_segment(&mut self, segment: Segment) {
        self.lease.reserve(segment.path.clone());
        self.segments.push(segment);
    }

    pub(super) fn started_at(&self) -> SystemTime {
        self.started_at
    }

    pub(super) fn is_sufficient(&self) -> bool {
        let ended_at = unix_time_millis(self.ended_at).unwrap_or_default();
        self.segments
            .iter()
            .any(|segment| segment.ended_at >= ended_at)
    }

    pub(super) fn into_job(self) -> ClipJob {
        ClipJob {
            event_id: Uuid::now_v7().to_string(),
            camera_id: self.camera_id,
            started_at: self.started_at,
            ended_at: self.ended_at,
            path: self.path,
            segments: self.segments,
            _lease: self.lease,
        }
    }
}
