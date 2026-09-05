use std::thread;

use gstreamer as gst;
use tokio::sync::mpsc;
use url::Url;

use super::{
    CameraStream, CameraStreamError, CameraStreamEvent, CameraStreamFuture, HlsConfig,
    SegmentRecordingConfig,
};

const EVENT_BUFFER_CAPACITY: usize = 8;

pub struct GstreamerCameraStream {
    receiver: mpsc::Receiver<Result<CameraStreamEvent, CameraStreamError>>,
}

impl GstreamerCameraStream {
    pub fn new(
        rtsp_url: String,
        recording: SegmentRecordingConfig,
        hls: HlsConfig,
    ) -> Result<Self, CameraStreamError> {
        let url = Url::parse(&rtsp_url).map_err(|_| CameraStreamError::Unavailable)?;
        if !matches!(url.scheme(), "rtsp" | "rtsps") {
            return Err(CameraStreamError::Unavailable);
        }
        gst::init().map_err(|_| CameraStreamError::Failed)?;

        let (sender, receiver) = mpsc::channel(EVENT_BUFFER_CAPACITY);
        thread::Builder::new()
            .name("camwatch-rtsp".to_owned())
            .spawn(move || super::gstreamer::run_worker(rtsp_url, recording, hls, sender))
            .map_err(|_| CameraStreamError::Failed)?;

        Ok(Self { receiver })
    }
}

impl CameraStream for GstreamerCameraStream {
    fn next_event(
        &mut self,
    ) -> CameraStreamFuture<'_, Result<CameraStreamEvent, CameraStreamError>> {
        Box::pin(async move {
            self.receiver
                .recv()
                .await
                .unwrap_or(Err(CameraStreamError::Unavailable))
        })
    }
}
