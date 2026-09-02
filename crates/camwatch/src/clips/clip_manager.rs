use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};

use dashmap::DashMap;
use tokio::sync::mpsc;

use crate::storage::{Database, Segment, StorageError, unix_time_millis};

use super::{ClipJob, active_clip::ActiveClip, segment_lease::SegmentLease};

type SegmentReservations = Arc<DashMap<String, usize>>;

pub struct ClipManager {
    clips: DashMap<String, ActiveClip>,
    database: Database,
    clip_sender: mpsc::UnboundedSender<ClipJob>,
    clips_directory: PathBuf,
    segment_reservations: SegmentReservations,
}

impl ClipManager {
    pub fn new(
        database: Database,
        clip_sender: mpsc::UnboundedSender<ClipJob>,
        clips_directory: PathBuf,
    ) -> Self {
        Self {
            clips: DashMap::new(),
            database,
            clip_sender,
            clips_directory,
            segment_reservations: Arc::new(DashMap::new()),
        }
    }

    pub async fn add_clip(
        &self,
        camera_id: String,
        detected_at: SystemTime,
        pre_duration: Duration,
        post_duration: Duration,
    ) -> Result<(), StorageError> {
        let mut clip = ActiveClip::new(
            camera_id.clone(),
            detected_at,
            pre_duration,
            post_duration,
            self.create_clip_path(&camera_id, detected_at),
            SegmentLease::new(Arc::clone(&self.segment_reservations)),
        );
        let past_segments = self
            .database
            .segments_overlapping(
                &camera_id,
                unix_time_millis(clip.started_at()).unwrap_or_default(),
                unix_time_millis(detected_at).unwrap_or_default(),
            )
            .await?;

        for segment in past_segments {
            clip.add_segment(segment);
        }

        self.clips.insert(camera_id, clip);
        Ok(())
    }

    pub fn put_segment_and_try_save_clip(&self, segment: Segment) {
        let camera_id = segment.camera_id.clone();
        let ready = {
            let Some(mut clip) = self.clips.get_mut(&camera_id) else {
                return;
            };

            clip.add_segment(segment);
            clip.is_sufficient()
        };

        if !ready {
            return;
        }

        let Some((_, clip)) = self.clips.remove(&camera_id) else {
            return;
        };
        let job = clip.into_job();

        if let Err(error) = self.clip_sender.send(job) {
            tracing::warn!(camera_id, "clip worker is unavailable");
            drop(error.0);
        }
    }

    pub fn is_camera_recording(&self, camera_id: &str) -> bool {
        self.clips.contains_key(camera_id)
    }

    pub fn is_segment_reserved(&self, path: &str) -> bool {
        self.segment_reservations.contains_key(path)
    }

    fn create_clip_path(&self, camera_id: &str, started_at: SystemTime) -> PathBuf {
        let filename = format!("{}.mp4", unix_time_millis(started_at).unwrap());
        self.clips_directory.join(camera_id).join(filename)
    }
}
