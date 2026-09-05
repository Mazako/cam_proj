use std::thread::{self, JoinHandle};

use gstreamer as gst;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use url::Url;

use super::{
    CameraStream, CameraStreamError, CameraStreamEvent, CameraStreamFuture, HlsConfig,
    SegmentRecordingConfig,
};

const EVENT_BUFFER_CAPACITY: usize = 8;

pub struct GstreamerCameraStream {
    receiver: mpsc::Receiver<Result<CameraStreamEvent, CameraStreamError>>,
    cancel: CancellationToken,
    worker: Option<JoinHandle<()>>,
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
        let cancel = CancellationToken::new();
        let worker_cancel = cancel.clone();
        let worker = thread::Builder::new()
            .name("camwatch-rtsp".to_owned())
            .spawn(move || {
                super::gstreamer::run_worker(rtsp_url, recording, hls, sender, worker_cancel)
            })
            .map_err(|_| CameraStreamError::Failed)?;

        Ok(Self {
            receiver,
            cancel,
            worker: Some(worker),
        })
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

    fn shutdown(&mut self) -> CameraStreamFuture<'_, ()> {
        let cancel = self.cancel.clone();
        let worker = self.worker.take();
        Box::pin(async move {
            cancel.cancel();
            if let Some(worker) = worker {
                let _ = tokio::task::spawn_blocking(move || worker.join()).await;
            }
        })
    }
}

impl Drop for GstreamerCameraStream {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
