use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use tokio::sync::mpsc;

use crate::storage::unix_time_millis;

use super::{ClipJob, ClipStoreError, Segment, active_clip::ActiveClip};

struct ClipState {
    clips: HashMap<String, ActiveClip>,
    segments: HashMap<PathBuf, Arc<Segment>>,
}

pub struct ClipManager {
    state: Mutex<ClipState>,
    clip_sender: mpsc::UnboundedSender<ClipJob>,
    clips_directory: PathBuf,
}

impl ClipManager {
    pub fn new(clip_sender: mpsc::UnboundedSender<ClipJob>, clips_directory: PathBuf) -> Self {
        Self {
            state: Mutex::new(ClipState {
                clips: HashMap::new(),
                segments: HashMap::new(),
            }),
            clip_sender,
            clips_directory,
        }
    }

    pub fn add_or_extend_clip(
        &self,
        camera_id: String,
        detected_at: SystemTime,
        pre_duration: Duration,
        post_duration: Duration,
    ) -> Result<(), ClipStoreError> {
        let mut state = self
            .state
            .lock()
            .expect("clip manager state should not be poisoned");

        if let Some(existing_clip) = state.clips.get_mut(&camera_id) {
            existing_clip.extend(detected_at + post_duration);
            return Ok(());
        }

        let mut clip = ActiveClip::new(
            camera_id.clone(),
            detected_at,
            pre_duration,
            post_duration,
            self.create_clip_path(&camera_id, detected_at),
        )?;
        let clip_started_at = clip.started_at();

        let mut past_segments = state
            .segments
            .values()
            .filter(|segment| {
                segment.camera_id == camera_id
                    && segment.started_at <= detected_at
                    && segment.ended_at >= clip_started_at
            })
            .cloned()
            .collect::<Vec<_>>();
        past_segments.sort_by(|left, right| {
            left.started_at
                .cmp(&right.started_at)
                .then_with(|| left.path.cmp(&right.path))
        });

        for segment in past_segments {
            clip.add_segment(segment);
        }

        state.clips.insert(camera_id, clip);
        Ok(())
    }

    pub fn register_segment(
        &self,
        camera_id: String,
        path: PathBuf,
        started_at: SystemTime,
        ended_at: SystemTime,
    ) -> Result<(), ClipStoreError> {
        if ended_at < started_at {
            return Err(ClipStoreError::InvalidTimeRange);
        }
        let path = fs::canonicalize(path).map_err(ClipStoreError::FileMetadata)?;
        let size_bytes = fs::metadata(&path)
            .map_err(ClipStoreError::FileMetadata)?
            .len();
        let segment = Arc::new(Segment {
            camera_id: camera_id.clone(),
            path: path.clone(),
            started_at,
            ended_at,
            size_bytes,
        });
        let job = {
            let mut state = self
                .state
                .lock()
                .expect("clip manager state should not be poisoned");
            if state.segments.contains_key(&path) {
                return Err(ClipStoreError::SegmentAlreadyRegistered);
            }
            state.segments.insert(path, Arc::clone(&segment));
            let ready = if let Some(clip) = state.clips.get_mut(&camera_id) {
                clip.add_segment(segment);
                clip.is_sufficient()
            } else {
                false
            };
            ready.then(|| state.clips.remove(&camera_id).map(|clip| clip.into_job()))
        }
        .flatten();

        if let Some(job) = job
            && self.clip_sender.send(job).is_err()
        {
            tracing::warn!(camera_id, "clip worker is unavailable");
        }
        Ok(())
    }

    pub fn save_if_has_clip(&self, camera_id: &str) -> Result<(), ClipStoreError> {
        let mut state = self
            .state
            .lock()
            .expect("clip manager state should not be poisoned");
        if let Some(clip) = state.clips.remove(camera_id)
            && clip.has_segments()
        {
            let job = clip.into_job();
            if self.clip_sender.send(job).is_err() {
                tracing::warn!(camera_id, "clip worker is unavailable");
            }
        }
        Ok(())
    }

    pub async fn retain_expired_segments(&self, rolling_buffer_seconds: u64) {
        let Some(before) =
            SystemTime::now().checked_sub(Duration::from_secs(rolling_buffer_seconds))
        else {
            return;
        };
        let expired = {
            let mut state = self
                .state
                .lock()
                .expect("clip manager state should not be poisoned");
            let paths = state
                .segments
                .iter()
                .filter(|(_, segment)| segment.ended_at < before && Arc::strong_count(segment) == 1)
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>();
            paths
                .into_iter()
                .filter_map(|path| state.segments.remove(&path))
                .collect::<Vec<_>>()
        };

        for segment in expired {
            match tokio::fs::remove_file(&segment.path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    tracing::warn!(path = %segment.path.display(), %error, "failed to remove segment file");
                    self.state
                        .lock()
                        .expect("clip manager state should not be poisoned")
                        .segments
                        .insert(segment.path.clone(), segment);
                }
            }
        }
    }

    fn create_clip_path(&self, camera_id: &str, started_at: SystemTime) -> PathBuf {
        let filename = format!("{}.mp4", unix_time_millis(started_at).unwrap());
        self.clips_directory.join(camera_id).join(filename)
    }
}
