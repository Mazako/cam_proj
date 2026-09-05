use gstreamer::{self as gst, prelude::*};

use super::segment_output::SegmentOutput;
use super::{PipelineError, SegmentRecordingConfig, escape_pipeline_value};

pub fn build_pipeline(
    rtsp_url: &str,
    recording: &SegmentRecordingConfig,
) -> Result<gst::Pipeline, PipelineError> {
    let description = recording_pipeline_description(rtsp_url, recording.output()?);
    let element = gst::parse::launch(&description).map_err(|_| PipelineError::Build)?;

    element
        .downcast::<gst::Pipeline>()
        .map_err(|_| PipelineError::Build)
}

fn recording_pipeline_description(rtsp_url: &str, output: SegmentOutput) -> String {
    let rtsp_url = escape_pipeline_value(rtsp_url);
    let location = escape_pipeline_value(output.location.to_string_lossy().as_ref());
    format!(
        concat!(
            "rtspsrc ",
            "name=rtsp_source ",
            "location=\"{rtsp_url}\" ",
            "protocols=tcp ",
            "latency=200 ",
            "tcp-timeout=5000000 ",
            "! rtph264depay ",
            "name=depayloader ",
            "! h264parse ",
            "name=parser ",
            "config-interval=-1 ",
            "! tee ",
            "name=encoded ",
            "encoded. ! queue ",
            "! avdec_h264 ",
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
        location = location,
        start_index = output.start_index,
        rotation_nanoseconds = output.rotation_nanoseconds,
    )
}
