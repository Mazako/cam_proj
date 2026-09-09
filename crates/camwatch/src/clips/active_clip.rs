use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

use crate::storage::unix_time_millis;

use super::{ClipJob, ClipStoreError, Segment};

pub(super) struct ActiveClip {
    camera_id: String,
    started_at: SystemTime,
    ended_at: SystemTime,
    path: PathBuf,
    segments: Vec<Arc<Segment>>,
}

impl ActiveClip {
    pub(super) fn new(
        camera_id: String,
        detected_at: SystemTime,
        pre_duration: Duration,
        post_duration: Duration,
        path: PathBuf,
    ) -> Result<Self, ClipStoreError> {
        let started_at = detected_at
            .checked_sub(pre_duration)
            .ok_or(ClipStoreError::InvalidTimeRange)?;
        let ended_at = detected_at
            .checked_add(post_duration)
            .ok_or(ClipStoreError::InvalidTimeRange)?;

        Ok(Self {
            camera_id,
            started_at,
            ended_at,
            path,
            segments: Vec::new(),
        })
    }

    pub(super) fn add_segment(&mut self, segment: Arc<Segment>) {
        self.segments.push(segment);
    }

    pub(super) fn started_at(&self) -> SystemTime {
        self.started_at
    }

    pub(super) fn is_sufficient(&self) -> bool {
        let ended_at = unix_time_millis(self.ended_at).unwrap_or_default();
        self.segments
            .iter()
            .any(|segment| unix_time_millis(segment.ended_at).unwrap_or_default() >= ended_at)
    }

    pub(super) fn into_job(self) -> ClipJob {
        ClipJob {
            event_id: Uuid::now_v7().to_string(),
            camera_id: self.camera_id,
            started_at: self.started_at,
            ended_at: self.ended_at,
            path: self.path,
            segments: self.segments,
        }
    }

    pub(super) fn extend(&mut self, new_ended_at: SystemTime) {
        self.ended_at = new_ended_at;
    }

    pub(super) fn has_segments(&self) -> bool {
        !self.segments.is_empty()
    }
}
