use std::time::Duration;

use camwatch::clips::{create_clip, store_segment};
use tempfile::tempdir;

use super::support::{
    RtspSession, camera_stream, database_with_camera, is_playable_mp4, wait_for_finalized_segment,
    wait_for_online_frame,
};

#[tokio::test]
async fn assembles_a_clip_from_persisted_rtsp_segments() {
    let session = RtspSession::start("clips", None).await;
    let directory = tempdir().expect("temporary directory should exist");
    let mut stream = camera_stream(session.url.clone(), &directory.path().join("segments"));
    let database = database_with_camera(directory.path()).await;

    wait_for_online_frame(&mut stream).await;
    let first = wait_for_finalized_segment(&mut stream).await;
    let second = wait_for_finalized_segment(&mut stream).await;

    for (path, started_at, ended_at) in [&first, &second] {
        store_segment(
            &database,
            "front-door",
            path.clone(),
            *started_at,
            *ended_at,
        )
        .await
        .expect("finalized segment should be stored");
    }

    let clip = create_clip(
        &database,
        "front-door",
        first.1,
        second.2,
        directory.path().join("clips/event-1.mp4"),
    )
    .await
    .expect("clip should be assembled");

    assert!(clip.path.is_file());
    assert!(clip.duration > Duration::ZERO);
    assert!(is_playable_mp4(&clip.path));
}
