use std::{fs, time::Duration};

use camwatch::stream::{
    CameraStreamError, GstreamerCameraStream, SegmentRecordingConfig, build_pipeline,
};
use gstreamer::{self as gst, prelude::*};
use tempfile::tempdir;

#[test]
fn builds_an_h264_recording_pipeline() {
    gst::init().expect("GStreamer should initialize");
    let directory = tempdir().expect("temporary directory should exist");
    let recording =
        SegmentRecordingConfig::new(directory.path().to_path_buf(), Duration::from_secs(2));

    let pipeline = build_pipeline("rtsp://camera.local/stream1", &recording)
        .expect("recording pipeline should build");

    assert!(pipeline.by_name("rtsp_source").is_some());
    assert!(pipeline.by_name("depayloader").is_some());
    assert!(pipeline.by_name("parser").is_some());
    assert!(pipeline.by_name("decoder").is_some());
    assert!(pipeline.by_name("analysis_sink").is_some());
    assert!(pipeline.by_name("segment_sink").is_some());
}

#[test]
fn builds_a_recording_branch_with_mp4_segments() {
    gst::init().expect("GStreamer should initialize");
    let directory = tempdir().expect("temporary directory should exist");
    let recording =
        SegmentRecordingConfig::new(directory.path().to_path_buf(), Duration::from_secs(2));

    let pipeline = build_pipeline("rtsp://camera.local/stream1", &recording)
        .expect("recording pipeline should build");
    let segment_sink = pipeline
        .by_name("segment_sink")
        .expect("segment sink should exist");

    assert_eq!(segment_sink.property::<u64>("max-size-time"), 2_000_000_000);
    assert_eq!(segment_sink.property::<u64>("max-size-bytes"), 0);
    assert!(segment_sink.property::<bool>("async-finalize"));
    assert!(segment_sink.property::<bool>("send-keyframe-requests"));
    assert_eq!(segment_sink.property::<i32>("start-index"), 0);
}

#[test]
fn recording_pipeline_continues_segment_indexes_after_a_restart() {
    gst::init().expect("GStreamer should initialize");
    let directory = tempdir().expect("temporary directory should exist");
    fs::write(directory.path().join("segment-0000000041.mp4"), []).expect("segment should exist");
    let recording =
        SegmentRecordingConfig::new(directory.path().to_path_buf(), Duration::from_secs(2));

    let pipeline = build_pipeline("rtsp://camera.local/stream1", &recording)
        .expect("recording pipeline should build");
    let segment_sink = pipeline
        .by_name("segment_sink")
        .expect("segment sink should exist");

    assert_eq!(segment_sink.property::<i32>("start-index"), 42);
}

#[test]
fn rejects_a_non_rtsp_url_without_starting_a_camera_worker() {
    let result = GstreamerCameraStream::new(
        "https://camera.local/live".to_owned(),
        SegmentRecordingConfig::new("not-used".into(), Duration::from_secs(2)),
    );

    assert_eq!(result.err(), Some(CameraStreamError::Unavailable));
}
