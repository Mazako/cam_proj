use std::sync::Arc;

use crate::{
    clips::store_segment,
    motion::Mog2MotionDetector,
    ports::{CameraStream, CameraStreamEvent, CameraStreamStatus, MotionDetector},
    storage::Database,
    stream::CameraStatusModel,
};

pub struct CameraRuntime<S> {
    camera_id: String,
    stream: S,
    status_model: Arc<CameraStatusModel>,
    database: Database,
    motion_detector: Mog2MotionDetector,
}

impl<S> CameraRuntime<S>
where
    S: CameraStream,
{
    pub fn new(
        camera_id: String,
        stream: S,
        status_model: Arc<CameraStatusModel>,
        database: Database,
    ) -> Self {
        let motion_detector = Mog2MotionDetector::new().unwrap();
        Self {
            camera_id,
            stream,
            status_model,
            database,
            motion_detector,
        }
    }

    pub async fn run(mut self) {
        loop {
            match self.stream.next_event().await {
                Ok(CameraStreamEvent::Status(status)) => {
                    self.status_model.update(&self.camera_id, status);
                    match status {
                        CameraStreamStatus::Online { .. } => {
                            tracing::info!(camera_id = self.camera_id, "camera stream is online");
                        }
                        CameraStreamStatus::Offline { .. } => {
                            tracing::warn!(camera_id = self.camera_id, "camera stream is offline");
                        }
                    }
                }
                Ok(CameraStreamEvent::Frame(frame)) => {
                    let detection = self.motion_detector.detect(&frame).unwrap();
                    if detection.largest_contour_area > 0.0 {
                        tracing::info!("MOTION DETECTED :D")
                    }
                }
                Ok(CameraStreamEvent::SegmentFinalized {
                    path,
                    started_at,
                    ended_at,
                }) => {
                    if let Err(error) =
                        store_segment(&self.database, &self.camera_id, path, started_at, ended_at)
                            .await
                    {
                        tracing::warn!(camera_id = self.camera_id, %error, "segment could not be stored");
                    }
                }
                Err(_) => {
                    tracing::warn!(camera_id = self.camera_id, "camera stream stopped");
                    return;
                }
            }
        }
    }
}
