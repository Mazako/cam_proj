use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};

use dashmap::{DashMap, mapref::entry::Entry};
use tokio::sync::mpsc;

use crate::storage::{Database, Segment, StorageError, unix_time_millis};

type SegmentReservations = Arc<DashMap<String, usize>>;

pub(crate) struct SegmentLease {
    reservations: SegmentReservations,
    paths: Vec<String>,
}

impl SegmentLease {
    fn new(reservations: SegmentReservations) -> Self {
        Self {
            reservations,
            paths: Vec::new(),
        }
    }

    fn reserve(&mut self, path: String) {
        if self.paths.contains(&path) {
            return;
        }

        self.reservations
            .entry(path.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        self.paths.push(path);
    }
}

impl Drop for SegmentLease {
    fn drop(&mut self) {
        for path in &self.paths {
            if let Entry::Occupied(mut entry) = self.reservations.entry(path.clone()) {
                if *entry.get() == 1 {
                    entry.remove();
                } else {
                    *entry.get_mut() -= 1;
                }
            }
        }
    }
}

struct ActiveClip {
    camera_id: String,
    started_at: SystemTime,
    ended_at: SystemTime,
    path: PathBuf,
    segments: Vec<Segment>,
    lease: SegmentLease,
}

impl ActiveClip {
    fn new(
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

    fn add_segment(&mut self, segment: Segment) {
        self.lease.reserve(segment.path.clone());
        self.segments.push(segment);
    }

    fn is_sufficient(&self) -> bool {
        let ended_at = unix_time_millis(self.ended_at).unwrap_or_default();
        self.segments
            .iter()
            .any(|segment| segment.ended_at >= ended_at)
    }

    fn into_job(self) -> ClipJob {
        ClipJob {
            camera_id: self.camera_id,
            started_at: self.started_at,
            ended_at: self.ended_at,
            path: self.path,
            segments: self.segments,
            _lease: self.lease,
        }
    }
}

pub struct ClipJob {
    pub camera_id: String,
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
    pub path: PathBuf,
    pub segments: Vec<Segment>,
    pub(crate) _lease: SegmentLease,
}

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
                unix_time_millis(clip.started_at).unwrap_or_default(),
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
