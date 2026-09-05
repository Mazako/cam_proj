use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::SystemTime,
};

use backon::{BackoffBuilder, ExponentialBuilder};
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::{self as gst_app};
use tokio::sync::mpsc;

use super::{
    CameraStreamError, CameraStreamEvent, CameraStreamStatus, Frame, HlsConfig, PixelFormat,
    SegmentRecordingConfig, build_pipeline, segment_times::SegmentTimes,
};

pub(super) fn run_worker(
    rtsp_url: String,
    recording: SegmentRecordingConfig,
    hls: HlsConfig,
    sender: mpsc::Sender<Result<CameraStreamEvent, CameraStreamError>>,
) {
    let reconnect_backoff = ExponentialBuilder::default()
        .with_jitter()
        .with_min_delay(std::time::Duration::from_secs(1))
        .with_max_delay(std::time::Duration::from_secs(30))
        .without_max_times();
    let mut backoff = reconnect_backoff.build();
    let mut attempt = 0_u64;

    loop {
        attempt += 1;
        tracing::info!(attempt, "starting GStreamer camera pipeline");
        let result = run_pipeline(&rtsp_url, &recording, &hls, &sender);
        let connected = result.is_ok();
        if let Err(error) = result {
            tracing::warn!(
                attempt,
                ?error,
                "GStreamer camera pipeline stopped with an error"
            );
        } else {
            tracing::info!(
                attempt,
                "GStreamer camera pipeline stopped after receiving frames"
            );
        }
        if connected {
            backoff = reconnect_backoff.build();
        }

        if sender
            .blocking_send(Ok(CameraStreamEvent::Status(CameraStreamStatus::Offline {
                since: SystemTime::now(),
            })))
            .is_err()
        {
            tracing::info!(attempt, "camera stream event receiver was closed");
            return;
        }

        let delay = backoff
            .next()
            .expect("reconnect backoff has no attempt limit");
        tracing::info!(
            attempt,
            delay_ms = delay.as_millis(),
            "camera stream is offline; reconnect scheduled"
        );
        thread::sleep(delay);
    }
}

fn run_pipeline(
    rtsp_url: &str,
    recording: &SegmentRecordingConfig,
    hls: &HlsConfig,
    sender: &mpsc::Sender<Result<CameraStreamEvent, CameraStreamError>>,
) -> Result<(), CameraStreamError> {
    let pipeline = build_pipeline(rtsp_url, recording, hls).map_err(|error| {
        tracing::error!(?error, "GStreamer camera pipeline could not be built");
        CameraStreamError::Failed
    })?;
    let appsink = pipeline
        .by_name("analysis_sink")
        .and_downcast::<gst_app::AppSink>()
        .ok_or_else(|| {
            tracing::error!("GStreamer camera pipeline has no analysis appsink");
            CameraStreamError::Failed
        })?;
    let event_sender = sender.clone();
    let received_frame = Arc::new(AtomicBool::new(false));
    let online_reported = Arc::clone(&received_frame);

    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = match sink.pull_sample() {
                    Ok(sample) => sample,
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            "GStreamer analysis appsink could not pull a sample"
                        );
                        return Err(gst::FlowError::Error);
                    }
                };
                let frame = match frame_from_sample(sample) {
                    Ok(frame) => frame,
                    Err(()) => {
                        tracing::warn!(
                            "GStreamer analysis sample could not be converted to a frame"
                        );
                        return Err(gst::FlowError::Error);
                    }
                };
                if !online_reported.swap(true, Ordering::Relaxed) {
                    tracing::info!("camera stream received its first frame");
                    match event_sender.try_send(Ok(CameraStreamEvent::Status(
                        CameraStreamStatus::Online {
                            since: SystemTime::now(),
                        },
                    ))) {
                        Err(mpsc::error::TrySendError::Closed(_)) => {
                            return Err(gst::FlowError::Eos);
                        }
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            tracing::warn!("camera stream status event buffer is full")
                        }
                    }
                }
                match event_sender.try_send(Ok(CameraStreamEvent::Frame(frame))) {
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        tracing::info!("camera stream event receiver was closed");
                        Err(gst::FlowError::Eos)
                    }
                    Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => Ok(gst::FlowSuccess::Ok),
                }
            })
            .build(),
    );

    pipeline.set_state(gst::State::Playing).map_err(|error| {
        tracing::error!(
            ?error,
            "GStreamer camera pipeline could not enter Playing state"
        );
        CameraStreamError::Unavailable
    })?;
    tracing::debug!("GStreamer camera pipeline is Playing");

    let bus = pipeline.bus().ok_or_else(|| {
        tracing::error!("GStreamer camera pipeline has no message bus");
        CameraStreamError::Failed
    })?;
    let mut segment_times = SegmentTimes::new(SystemTime::now());
    let result = loop {
        let Some(message) = bus.timed_pop(gst::ClockTime::NONE) else {
            break Err(CameraStreamError::Unavailable);
        };

        match message.view() {
            gst::MessageView::Eos(..) => {
                tracing::warn!("GStreamer camera pipeline reached end of stream");
                break Err(CameraStreamError::Unavailable);
            }
            gst::MessageView::Error(error) => {
                let source = error.src().map(|source| source.name().to_string());
                tracing::error!(
                    source = ?source,
                    error = %error.error(),
                    debug = ?error.debug(),
                    "GStreamer camera pipeline reported an error"
                );
                break Err(CameraStreamError::Unavailable);
            }
            gst::MessageView::Warning(warning) => {
                let source = warning.src().map(|source| source.name().to_string());
                let warning_error = warning.error();
                if source.as_deref() == Some("rtsp_source")
                    && warning_error.matches(gst::ResourceError::Read)
                {
                    tracing::warn!(
                        source = ?source,
                        warning = %warning_error,
                        debug = ?warning.debug(),
                        "RTSP server closed the connection; camera stream is offline"
                    );
                    break Err(CameraStreamError::Unavailable);
                }
                tracing::warn!(
                    source = ?source,
                    warning = %warning_error,
                    debug = ?warning.debug(),
                    "GStreamer camera pipeline reported a warning"
                );
            }
            gst::MessageView::Element(element) => {
                if let Some(event) = segment_times.handle(element)
                    && {
                        tracing::debug!(event = ?event, "GStreamer camera pipeline finalized a segment");
                        sender.blocking_send(Ok(event)).is_err()
                    }
                {
                    tracing::info!("camera stream event receiver was closed");
                    break Err(CameraStreamError::Unavailable);
                }
            }
            _ => {}
        }
    };

    let _ = pipeline.set_state(gst::State::Null);
    tracing::debug!("GStreamer camera pipeline set to Null state");
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
