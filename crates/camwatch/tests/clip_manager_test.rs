use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};

use camwatch::clips::{ClipJob, ClipManager};
use tempfile::TempDir;
use tokio::sync::mpsc::{self, UnboundedReceiver};

const CAMERA_ID: &str = "front-door";

#[tokio::test]
async fn keeps_segments_until_the_clip_job_is_dropped() {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let (manager, mut receiver) = manager(&directory);
    let pre_event = register_segment(&manager, &directory, "pre-event.mp4", 90, 95);

    manager
        .add_or_extend_clip(
            CAMERA_ID.to_owned(),
            at(100),
            Duration::from_secs(10),
            Duration::from_secs(10),
        )
        .expect("clip should start");
    let post_event = register_segment(&manager, &directory, "post-event.mp4", 100, 110);
    let job = receiver.try_recv().expect("clip job should be queued");

    assert_eq!(paths(&job), vec![pre_event.clone(), post_event.clone()]);

    manager.retain_expired_segments(0).await;
    assert!(pre_event.is_file());
    assert!(post_event.is_file());

    drop(job);
    manager.retain_expired_segments(0).await;
    assert!(!pre_event.exists());
    assert!(!post_event.exists());
}

#[tokio::test]
async fn keeps_shared_segments_until_every_clip_job_is_dropped() {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let (manager, mut receiver) = manager(&directory);
    let shared = register_segment(&manager, &directory, "shared.mp4", 100, 105);

    manager
        .add_or_extend_clip(
            CAMERA_ID.to_owned(),
            at(100),
            Duration::ZERO,
            Duration::from_secs(10),
        )
        .expect("first clip should start");
    register_segment(&manager, &directory, "first-final.mp4", 105, 110);
    let first_job = receiver
        .try_recv()
        .expect("first clip job should be queued");

    manager
        .add_or_extend_clip(
            CAMERA_ID.to_owned(),
            at(105),
            Duration::from_secs(5),
            Duration::from_secs(10),
        )
        .expect("second clip should start");
    register_segment(&manager, &directory, "second-final.mp4", 110, 120);
    let second_job = receiver
        .try_recv()
        .expect("second clip job should be queued");

    assert!(paths(&first_job).contains(&shared));
    assert!(paths(&second_job).contains(&shared));

    drop(first_job);
    manager.retain_expired_segments(0).await;
    assert!(shared.is_file());

    drop(second_job);
    manager.retain_expired_segments(0).await;
    assert!(!shared.exists());
}

#[tokio::test]
async fn releases_segments_when_the_clip_worker_is_unavailable() {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let (manager, receiver) = manager(&directory);
    drop(receiver);

    manager
        .add_or_extend_clip(
            CAMERA_ID.to_owned(),
            at(100),
            Duration::ZERO,
            Duration::from_secs(10),
        )
        .expect("clip should start");
    let segment = register_segment(&manager, &directory, "worker-unavailable.mp4", 100, 110);

    manager.retain_expired_segments(0).await;
    assert!(!segment.exists());
}

fn manager(directory: &TempDir) -> (Arc<ClipManager>, UnboundedReceiver<ClipJob>) {
    let (sender, receiver) = mpsc::unbounded_channel();
    let manager = Arc::new(ClipManager::new(sender, directory.path().join("clips")));
    (manager, receiver)
}

fn register_segment(
    manager: &ClipManager,
    directory: &TempDir,
    name: &str,
    started_at: u64,
    ended_at: u64,
) -> PathBuf {
    let path = directory.path().join(name);
    std::fs::write(&path, b"segment").expect("segment file should be written");
    manager
        .register_segment(
            CAMERA_ID.to_owned(),
            path.clone(),
            at(started_at),
            at(ended_at),
        )
        .expect("segment should register");
    std::fs::canonicalize(path).expect("segment path should canonicalize")
}

fn paths(job: &ClipJob) -> Vec<PathBuf> {
    job.segments
        .iter()
        .map(|segment| segment.path.clone())
        .collect()
}

fn at(seconds: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
}
