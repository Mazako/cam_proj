use gstreamer::{self as gst, prelude::*};
use thiserror::Error;

use super::RtspCodec;
use super::segment_recording::{SegmentOutput, SegmentRecordingConfig, SegmentRecordingError};

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error(transparent)]
    Recording(#[from] SegmentRecordingError),
    #[error("GStreamer pipeline could not be built")]
    Build,
}

pub fn build_pipeline(
    rtsp_url: &str,
    codec: RtspCodec,
    recording: &SegmentRecordingConfig,
) -> Result<gst::Pipeline, PipelineError> {
    let description = recording_pipeline_description(rtsp_url, codec, recording.output()?);
    let element = gst::parse::launch(&description).map_err(|_| PipelineError::Build)?;

    element
        .downcast::<gst::Pipeline>()
        .map_err(|_| PipelineError::Build)
}

fn recording_pipeline_description(
    rtsp_url: &str,
    codec: RtspCodec,
    output: SegmentOutput,
) -> String {
    let rtsp_url = escape_pipeline_value(rtsp_url);
    let location = escape_pipeline_value(output.location.to_string_lossy().as_ref());
    let (depayloader, parser, decoder) = codec_elements(codec);

    format!(
        concat!(
            "rtspsrc ",
            "name=rtsp_source ",
            "location=\"{rtsp_url}\" ",
            "protocols=tcp ",
            "latency=200 ",
            "tcp-timeout=5000000 ",
            "! {depayloader} ",
            "name=depayloader ",
            "! {parser} ",
            "name=parser ",
            "config-interval=-1 ",
            "! tee ",
            "name=encoded ",
            "encoded. ! queue ",
            "! {decoder} ",
            "name=decoder ",
            "! videoconvert ",
            "! video/x-raw,format=BGR ",
            "! appsink ",
            "name=analysis_sink ",
            "sync=false ",
            "max-buffers=1 ",
            "drop=true ",
            "encoded. ! queue ",
            "! splitmuxsink ",
            "name=segment_sink ",
            "location=\"{location}\" ",
            "start-index={start_index} ",
            "max-size-time={rotation_nanoseconds} ",
            "max-size-bytes=0 ",
            "async-finalize=true ",
            "muxer-factory=mp4mux ",
            "send-keyframe-requests=true",
        ),
        rtsp_url = rtsp_url,
        depayloader = depayloader,
        parser = parser,
        decoder = decoder,
        location = location,
        start_index = output.start_index,
        rotation_nanoseconds = output.rotation_nanoseconds,
    )
}

fn codec_elements(codec: RtspCodec) -> (&'static str, &'static str, &'static str) {
    match codec {
        RtspCodec::H264 => ("rtph264depay", "h264parse", "avdec_h264"),
        RtspCodec::H265 => ("rtph265depay", "h265parse", "avdec_h265"),
    }
}

fn escape_pipeline_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
