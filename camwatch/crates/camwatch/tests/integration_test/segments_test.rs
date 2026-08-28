use tempfile::tempdir;

use super::support::{
    RtspSession, camera_stream, is_playable_mp4, wait_for_finalized_segment, wait_for_online_frame,
};

#[tokio::test]
async fn writes_playable_mp4_segments_from_the_rtsp_stream() {
    let session = RtspSession::start("recording", None).await;
    let directory = tempdir().expect("temporary directory should exist");
    let mut stream = camera_stream(session.url.clone(), directory.path());

    wait_for_online_frame(&mut stream).await;
    let (segment, _, _) = wait_for_finalized_segment(&mut stream).await;

    assert!(segment.starts_with(directory.path()));
    assert!(is_playable_mp4(&segment));
}
