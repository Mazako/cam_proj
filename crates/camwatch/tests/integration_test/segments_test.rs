use std::{fs, path::Path, time::Duration};

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

    let playlist = directory.path().join("hls/index.m3u8");
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if hls_playlist_has_segment(&playlist) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("HLS stream should produce a playlist and segment");
}

fn hls_playlist_has_segment(playlist: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(playlist) else {
        return false;
    };
    contents.lines().any(|line| {
        line.starts_with("segment-")
            && line.ends_with(".ts")
            && playlist
                .parent()
                .is_some_and(|directory| directory.join(line).is_file())
    })
}
