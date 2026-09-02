use std::{sync::Arc, time::Duration};

use crate::{
    clips::{ClipManager, store_segment},
    config::{AppConfig, CameraConfig},
    motion::{Mog2MotionDetector, MotionDetector, YoloAnalyzer},
    onvif::OnvifConnection,
    storage::Database,
    stream::{CameraStatusModel, CameraStream, CameraStreamEvent, CameraStreamStatus, Frame},
};

pub struct CameraRuntime<S> {
    pub pre_event_seconds: u64,
    pub post_event_seconds: u64,
    camera_config: CameraConfig,
    stream: S,
    status_model: Arc<CameraStatusModel>,
    database: Database,
    motion_detector: Mog2MotionDetector,
    clip_manager: Arc<ClipManager>,
    yolo_analyzer: Option<YoloAnalyzer>,
    onvif: Option<OnvifConnection>,
}

impl<S> CameraRuntime<S>
where
    S: CameraStream,
{
    pub async fn new(
        camera_config: CameraConfig,
        app_config: &AppConfig,
        stream: S,
        status_model: Arc<CameraStatusModel>,
        database: Database,
        clip_manager: Arc<ClipManager>,
    ) -> Self {
        let motion_detector = Mog2MotionDetector::new().unwrap();
        let yolo_analyzer = if camera_config.clip_after_motion {
            None
        } else {
            Some(YoloAnalyzer::new(camera_config.yolo_confidence).unwrap())
        };
        let onvif = OnvifConnection::try_build(&camera_config).await;
        Self {
            pre_event_seconds: u64::from(app_config.pre_event_seconds),
            post_event_seconds: u64::from(app_config.post_event_seconds),
            camera_config,
            stream,
            status_model,
            database,
            motion_detector,
            clip_manager,
            yolo_analyzer,
            onvif,
        }
    }

    pub fn has_ptz(&self) -> bool {
        self.onvif.is_some()
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
                Ok(CameraStreamEvent::Frame(frame)) => match self
                    .clip_manager
                    .is_camera_recording(self.camera_config.id.as_str())
                {
                    true => {}
                    false => {
                        let clip_triggered = if self.camera_config.clip_after_motion {
                            self.is_motion_detected(&frame)
                        } else {
                            self.is_motion_detected(&frame) && self.is_yolo_motion_detected(&frame)
                        };
                        if clip_triggered
                            && let Err(error) = self
                                .clip_manager
                                .add_clip(
                                    self.camera_config.id.as_str().to_owned(),
                                    frame.captured_at,
                                    Duration::from_secs(self.pre_event_seconds),
                                    Duration::from_secs(self.post_event_seconds),
                                )
                                .await
                        {
                            tracing::warn!(camera_id = self.camera_config.id.as_str(), %error, "clip could not be started");
                        }
                    }
                },
                Ok(CameraStreamEvent::SegmentFinalized {
                    path,
                    started_at,
                    ended_at,
                }) => {
                    match store_segment(
                        &self.database,
                        self.camera_config.id.as_str(),
                        path,
                        started_at,
                        ended_at,
                    )
                    .await
                    {
                        Err(err) => {
                            tracing::warn!(camera_id = self.camera_config.id.as_str(), %err, "segment could not be stored");
                        }
                        Ok(segment) => {
                            self.clip_manager.put_segment_and_try_save_clip(segment);
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
