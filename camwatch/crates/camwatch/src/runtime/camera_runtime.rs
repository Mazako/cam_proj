use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};

use tokio::sync::mpsc;

use crate::{
    clips::{clip_store::ClipCreationEvent, store_segment},
    config::{AppConfig, CameraConfig},
    motion::{Mog2MotionDetector, YoloAnalyzer},
    ports::{CameraStream, CameraStreamEvent, CameraStreamStatus, Frame, MotionDetector},
    storage::{Database, unix_time_millis},
    stream::CameraStatusModel,
};

pub struct CameraRuntime<S> {
    pub pre_event_seconds: u64,
    pub post_event_seconds: u64,
    pub clips_directory: PathBuf,
    camera_config: CameraConfig,
    stream: S,
    status_model: Arc<CameraStatusModel>,
    database: Database,
    motion_detector: Mog2MotionDetector,
    clip_sender: mpsc::UnboundedSender<ClipCreationEvent>,
    active_clip: Option<ActiveClip>,
    yolo_analyzer: Option<YoloAnalyzer>,
}

impl<S> CameraRuntime<S>
where
    S: CameraStream,
{
    pub fn new(
        camera_config: CameraConfig,
        app_config: &AppConfig,
        stream: S,
        status_model: Arc<CameraStatusModel>,
        database: Database,
        clip_sender: mpsc::UnboundedSender<ClipCreationEvent>,
    ) -> Self {
        let motion_detector = Mog2MotionDetector::new().unwrap();
        let yolo_analyzer = if camera_config.clip_after_motion {
            None
        } else {
            Some(YoloAnalyzer::new(camera_config.yolo_confidence).unwrap())
        };
        Self {
            pre_event_seconds: u64::from(app_config.pre_event_seconds),
            post_event_seconds: u64::from(app_config.post_event_seconds),
            clips_directory: app_config.clips_directory.clone(),
            camera_config,
            stream,
            status_model,
            database,
            motion_detector,
            clip_sender,
            active_clip: None,
            yolo_analyzer,
        }
    }

    pub async fn run(mut self) {
        loop {
            match self.stream.next_event().await {
                Ok(CameraStreamEvent::Status(status)) => {
                    self.status_model
                        .update(self.camera_config.id.as_str(), status);
                    match status {
                        CameraStreamStatus::Online { .. } => {
                            tracing::info!(
                                camera_id = self.camera_config.id.as_str(),
                                "camera stream is online"
                            );
                        }
                        CameraStreamStatus::Offline { .. } => {
                            tracing::warn!(
                                camera_id = self.camera_config.id.as_str(),
                                "camera stream is offline"
                            );
                        }
                    }
                }
                Ok(CameraStreamEvent::Frame(frame)) => match &mut self.active_clip {
                    Some(_) => {}
                    None => {
                        let clip_triggered = if self.camera_config.clip_after_motion {
                            self.is_motion_detected(&frame)
                        } else {
                            self.is_motion_detected(&frame) && self.is_yolo_motion_detected(&frame)
                        };
                        if clip_triggered {
                            let started_at = frame.captured_at;
                            let path = self.create_clip_path(started_at);
                            self.active_clip = Some(ActiveClip::new(
                                started_at,
                                Duration::from_secs(self.pre_event_seconds),
                                Duration::from_secs(self.post_event_seconds),
                                path,
                            ));
                        }
                    }
                },
                Ok(CameraStreamEvent::SegmentFinalized {
                    path,
                    started_at,
                    ended_at,
                }) => {
                    if let Err(error) = store_segment(
                        &self.database,
                        self.camera_config.id.as_str(),
                        path,
                        started_at,
                        ended_at,
                    )
                    .await
                    {
                        tracing::warn!(camera_id = self.camera_config.id.as_str(), %error, "segment could not be stored");
                    } else if let Some(clip) = &self.active_clip
                        && clip.is_sufficient(ended_at)
                    {
                        let event = clip.to_event(self.camera_config.id.as_str().to_owned());
                        if let Err(error) = self.clip_sender.send(event) {
                            tracing::warn!(camera_id = self.camera_config.id.as_str(), %error, "clip could not be sent to the clip worker");
                        }
                        self.active_clip = None;
                    }
                }
                Err(_) => {
                    tracing::warn!(
                        camera_id = self.camera_config.id.as_str(),
                        "camera stream stopped"
                    );
                    return;
                }
            }
        }
    }

    fn create_clip_path(&self, started_at: SystemTime) -> PathBuf {
        let filename = format!("{}.mp4", unix_time_millis(started_at).unwrap());
        self.clips_directory
            .join(self.camera_config.id.as_str())
            .join(filename)
    }

    fn is_motion_detected(&mut self, frame: &Frame) -> bool {
        let detection = self.motion_detector.detect(frame).unwrap();
        detection.largest_contour_area > 0.0
    }

    fn is_yolo_motion_detected(&mut self, frame: &Frame) -> bool {
        let Some(yolo_analyzer) = self.yolo_analyzer.as_mut() else {
            return false;
        };
        let detections = yolo_analyzer.analyze(frame).unwrap();
        !detections.is_empty()
    }
}

struct ActiveClip {
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
    pub path: PathBuf,
}

impl ActiveClip {
    fn new(
        detected_at: SystemTime,
        pre_duration: Duration,
        post_duration: Duration,
        path: PathBuf,
    ) -> Self {
        let started_at = detected_at.checked_sub(pre_duration).unwrap();
        let ended_at = detected_at.checked_add(post_duration).unwrap();
        Self {
            started_at,
            ended_at,
            path,
        }
    }

    fn to_event(&self, camera_id: String) -> ClipCreationEvent {
        ClipCreationEvent::new(camera_id, self.started_at, self.ended_at, self.path.clone())
    }

    fn is_sufficient(&self, time: SystemTime) -> bool {
        time >= self.ended_at
    }
}
