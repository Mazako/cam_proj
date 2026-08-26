use camwatch::{
    ports::CameraStreamError,
    stream::{GstreamerCameraStream, RtspCodec, pipeline_description},
};

#[test]
fn builds_an_h264_tcp_pipeline_for_analysis_frames() {
    let description = pipeline_description("rtsp://camera.local/stream1", RtspCodec::H264);

    assert!(description.contains(
        "rtspsrc location=\"rtsp://camera.local/stream1\" protocols=tcp latency=200 tcp-timeout=5000000"
    ));
    assert!(description.contains("rtph264depay ! h264parse ! avdec_h264"));
    assert!(description.contains("video/x-raw,format=BGR"));
    assert!(description.contains("appsink name=analysis_sink"));
}

#[test]
fn builds_an_h265_tcp_pipeline_for_analysis_frames() {
    let description = pipeline_description("rtsp://camera.local/stream1", RtspCodec::H265);

    assert!(description.contains("rtph265depay ! h265parse ! avdec_h265"));
    assert!(!description.contains("rtph264depay"));
}

#[test]
fn escapes_a_quote_in_the_rtsp_location() {
    let description = pipeline_description("rtsp://camera.local/stream\\\"1", RtspCodec::H264);

    assert!(description.contains("location=\"rtsp://camera.local/stream\\\\\\\"1\""));
}

#[test]
fn rejects_a_non_rtsp_url_without_starting_a_camera_worker() {
    let result =
        GstreamerCameraStream::new("https://camera.local/live".to_owned(), RtspCodec::H264);

    assert_eq!(result.err(), Some(CameraStreamError::Unavailable));
}
