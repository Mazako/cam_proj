use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};

use tokio::sync::mpsc;

use crate::{
    clips::{clip_store::ClipCreationEvent, store_segment},
    config::{AppConfig, CameraConfig},
    motion::Mog2MotionDetector,
    ports::{CameraStream, CameraStreamEvent, CameraStreamStatus, MotionDetector},
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
        }
    }

    pub async fn run(mut self) {
        let mut current_detection: Option<SystemTime> = None;
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
                Ok(CameraStreamEvent::Frame(frame)) => {
                    let detection = self.motion_detector.detect(&frame).unwrap();
                    if detection.largest_contour_area > 0.0 {
                        match current_detection {
                            Some(_) => {}
                            None => current_detection = Some(SystemTime::now()),
                        }
                        tracing::info!("MOTION DETECTED :D");
                    }
                }
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
                    } else if let Some(time) = current_detection {
                        let post_time = time
                            .checked_add(Duration::from_secs(self.post_event_seconds))
                            .unwrap();
                        if ended_at >= post_time {
                            let path = self
                                .clips_directory
                                .join(self.camera_config.id.as_str())
                                .join(unix_time_millis(time).unwrap().to_string());
                            let pre_time = time
                                .checked_sub(Duration::from_secs(self.pre_event_seconds))
                                .unwrap();
                            if let Err(error) = self.clip_sender.send(ClipCreationEvent::new(
                                self.camera_config.id.as_str().to_string(),
                                pre_time,
                                post_time,
                                path,
                            )) {
                                tracing::error!(
                                    camera_id = self.camera_config.id.as_str(),
                                    %error,
                                    "clip creation event could not be queued"
                                );
                            }
                            current_detection = None;
                        }
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
}
