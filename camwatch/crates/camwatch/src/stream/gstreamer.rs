use std::{
    collections::HashMap,
    env,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, SystemTime},
};

use backon::{BackoffBuilder, ExponentialBuilder};
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::{self as gst_app};
use tokio::sync::mpsc;
use url::Url;

use derive_new::new;

use super::{
    CameraStream, CameraStreamError, CameraStreamEvent, CameraStreamFuture, CameraStreamStatus,
    Frame, PixelFormat, RtspCodec, SegmentRecordingConfig, build_pipeline,
};

const EVENT_BUFFER_CAPACITY: usize = 8;

#[derive(new)]
struct SegmentTimes {
    pipeline_started_at: SystemTime,
    #[new(default)]
    opened_at: HashMap<PathBuf, SystemTime>,
}

impl SegmentTimes {
    fn handle(&mut self, element: &gst::message::Element) -> Option<CameraStreamEvent> {
        let structure = element.structure()?;
        let location = structure.get::<String>("location").ok()?;
        let running_time = structure.get::<gst::ClockTime>("running-time").ok()?;
        let at = self
            .pipeline_started_at
            .checked_add(Duration::from_nanos(running_time.nseconds()))?;
        let path = PathBuf::from(location);

        match structure.name().as_str() {
            "splitmuxsink-fragment-opened" => {
                self.opened_at.insert(path, at);
                None
            }
            "splitmuxsink-fragment-closed" => {
                let started_at = self.opened_at.remove(&path)?;
                Some(CameraStreamEvent::SegmentFinalized {
                    path,
                    started_at,
                    ended_at: at,
                })
            }
            _ => None,
        }
    }
}

pub struct GstreamerCameraStream {
    receiver: mpsc::Receiver<Result<CameraStreamEvent, CameraStreamError>>,
}

impl GstreamerCameraStream {
    pub fn from_environment(
        rtsp_url_env: &str,
        codec: RtspCodec,
        recording: SegmentRecordingConfig,
    ) -> Result<Self, CameraStreamError> {
        let rtsp_url = env::var(rtsp_url_env).map_err(|_| CameraStreamError::Unavailable)?;
        Self::new(rtsp_url, codec, recording)
    }

    pub fn new(
        rtsp_url: String,
        codec: RtspCodec,
        recording: SegmentRecordingConfig,
    ) -> Result<Self, CameraStreamError> {
        let url = Url::parse(&rtsp_url).map_err(|_| CameraStreamError::Unavailable)?;
        if !matches!(url.scheme(), "rtsp" | "rtsps") {
            return Err(CameraStreamError::Unavailable);
        }
        gst::init().map_err(|_| CameraStreamError::Failed)?;

        let (sender, receiver) = mpsc::channel(EVENT_BUFFER_CAPACITY);
        thread::Builder::new()
            .name("camwatch-rtsp".to_owned())
            .spawn(move || run_worker(rtsp_url, codec, recording, sender))
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

fn run_worker(
    rtsp_url: String,
    codec: RtspCodec,
    recording: SegmentRecordingConfig,
    sender: mpsc::Sender<Result<CameraStreamEvent, CameraStreamError>>,
) {
    let reconnect_backoff = ExponentialBuilder::default()
        .with_jitter()
        .with_min_delay(std::time::Duration::from_secs(1))
        .with_max_delay(std::time::Duration::from_secs(30))
        .without_max_times();
    let mut backoff = reconnect_backoff.build();

    loop {
        let connected = run_pipeline(&rtsp_url, codec, &recording, &sender).is_ok();
        if connected {
            backoff = reconnect_backoff.build();
        }

        if sender
            .blocking_send(Ok(CameraStreamEvent::Status(CameraStreamStatus::Offline {
                since: SystemTime::now(),
            })))
            .is_err()
        {
            return;
        }

        let delay = backoff
            .next()
            .expect("reconnect backoff has no attempt limit");
        thread::sleep(delay);
    }
}

fn run_pipeline(
    rtsp_url: &str,
    codec: RtspCodec,
    recording: &SegmentRecordingConfig,
    sender: &mpsc::Sender<Result<CameraStreamEvent, CameraStreamError>>,
) -> Result<(), CameraStreamError> {
    let pipeline =
        build_pipeline(rtsp_url, codec, recording).map_err(|_| CameraStreamError::Failed)?;
    let appsink = pipeline
        .by_name("analysis_sink")
        .and_downcast::<gst_app::AppSink>()
        .ok_or(CameraStreamError::Failed)?;
    let event_sender = sender.clone();
    let received_frame = Arc::new(AtomicBool::new(false));
    let online_reported = Arc::clone(&received_frame);

    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Error)?;
                let frame = frame_from_sample(sample).map_err(|_| gst::FlowError::Error)?;
                if !online_reported.swap(true, Ordering::Relaxed) {
                    match event_sender.try_send(Ok(CameraStreamEvent::Status(
                        CameraStreamStatus::Online {
                            since: SystemTime::now(),
                        },
                    ))) {
                        Err(mpsc::error::TrySendError::Closed(_)) => {
                            return Err(gst::FlowError::Eos);
                        }
                        Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => {}
                    }
                }
                match event_sender.try_send(Ok(CameraStreamEvent::Frame(frame))) {
                    Err(mpsc::error::TrySendError::Closed(_)) => Err(gst::FlowError::Eos),
                    Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => Ok(gst::FlowSuccess::Ok),
                }
            })
            .build(),
    );

    pipeline
        .set_state(gst::State::Playing)
        .map_err(|_| CameraStreamError::Unavailable)?;

    let bus = pipeline.bus().ok_or(CameraStreamError::Failed)?;
    let mut segment_times = SegmentTimes::new(SystemTime::now());
    let result = loop {
        let Some(message) = bus.timed_pop(gst::ClockTime::NONE) else {
            break Err(CameraStreamError::Unavailable);
        };

        match message.view() {
            gst::MessageView::Eos(..) | gst::MessageView::Error(..) => {
                break Err(CameraStreamError::Unavailable);
            }
            gst::MessageView::Element(element) => {
                if let Some(event) = segment_times.handle(element)
                    && sender.blocking_send(Ok(event)).is_err()
                {
                    break Err(CameraStreamError::Unavailable);
                }
            }
            _ => {}
        }
    };

    let _ = pipeline.set_state(gst::State::Null);
    if received_frame.load(Ordering::Relaxed) {
        Ok(())
    } else {
        result
    }
}

fn frame_from_sample(sample: gst::Sample) -> Result<Frame, ()> {
    let caps = sample.caps().ok_or(())?;
    let structure = caps.structure(0).ok_or(())?;
    let width = structure.get::<i32>("width").map_err(|_| ())?;
    let height = structure.get::<i32>("height").map_err(|_| ())?;
    let buffer = sample.buffer().ok_or(())?;
    let map = buffer.map_readable().map_err(|_| ())?;

    Ok(Frame::new(
        map.as_slice().to_vec(),
        width.try_into().map_err(|_| ())?,
        height.try_into().map_err(|_| ())?,
        PixelFormat::Bgr8,
        SystemTime::now(),
    ))
}
