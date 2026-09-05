use gstreamer::{self as gst, prelude::*};

use super::segment_output::SegmentOutput;
use super::{
    HlsConfig, PipelineError, SegmentRecordingConfig, escape_pipeline_value,
    hls::{self, HlsOutput},
};

pub fn build_pipeline(
    rtsp_url: &str,
    recording: &SegmentRecordingConfig,
    hls: &HlsConfig,
) -> Result<gst::Pipeline, PipelineError> {
    let description = recording_pipeline_description(rtsp_url, recording.output()?, hls.output()?);
    let element = gst::parse::launch(&description).map_err(|_| PipelineError::Build)?;

    element
        .downcast::<gst::Pipeline>()
        .map_err(|_| PipelineError::Build)
}

fn recording_pipeline_description(rtsp_url: &str, output: SegmentOutput, hls: HlsOutput) -> String {
    let rtsp_url = escape_pipeline_value(rtsp_url);
    let location = escape_pipeline_value(output.location.to_string_lossy().as_ref());
    let hls_playlist_location =
        escape_pipeline_value(hls.playlist_location.to_string_lossy().as_ref());
    let hls_segment_location =
        escape_pipeline_value(hls.segment_location.to_string_lossy().as_ref());
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
            "! tee ",
            "name=encoded ",
            "encoded. ! queue ",
            "! h264parse ",
            "name=analysis_parser ",
            "config-interval=-1 ",
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
            "! h264parse ",
            "name=recording_parser ",
            "config-interval=-1 ",
            "! splitmuxsink ",
            "name=segment_sink ",
            "location=\"{location}\" ",
            "start-index={start_index} ",
            "max-size-time={rotation_nanoseconds} ",
            "max-size-bytes=0 ",
            "async-finalize=true ",
            "muxer-factory=mp4mux ",
            "send-keyframe-requests=true ",
            "encoded. ! queue ",
            "! h264parse ",
            "name=hls_parser ",
            "config-interval=-1 ",
            "! hlssink2 ",
            "name=hls_sink ",
            "target-duration={hls_target_duration} ",
            "playlist-length={hls_playlist_length} ",
            "max-files={hls_max_files} ",
            "playlist-location=\"{hls_playlist_location}\" ",
            "location=\"{hls_segment_location}\"",
        ),
        rtsp_url = rtsp_url,
        location = location,
        hls_target_duration = hls::TARGET_DURATION_SECONDS,
        hls_playlist_length = hls::PLAYLIST_LENGTH,
        hls_max_files = hls::MAX_FILES,
        hls_playlist_location = hls_playlist_location,
        hls_segment_location = hls_segment_location,
        start_index = output.start_index,
        rotation_nanoseconds = output.rotation_nanoseconds,
    )
}
