use std::{sync::Arc, time::Duration};

use camwatch::clips::{ClipManager, create_retainer_worker};
use tempfile::TempDir;
use tokio::sync::mpsc;

const CAMERA_ID: &str = "front-door";

#[tokio::test]
async fn removes_expired_segments_and_keeps_fresh_ones() {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let manager = manager(&directory);
    let expired = register_segment(&manager, &directory, "expired.mp4", Duration::from_secs(90));
    let fresh = register_segment(&manager, &directory, "fresh.mp4", Duration::from_secs(5));

    manager.retain_expired_segments(30).await;

    assert!(!expired.exists());
    assert!(fresh.is_file());
}

#[tokio::test]
async fn retainer_worker_uses_clip_manager_as_its_only_segment_source() {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let manager = manager(&directory);
    let expired = register_segment(&manager, &directory, "expired.mp4", Duration::from_secs(90));

    create_retainer_worker(1, Arc::clone(&manager));
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if !expired.exists() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("retainer should remove the expired segment");
}

fn manager(directory: &TempDir) -> Arc<ClipManager> {
    let (sender, _receiver) = mpsc::unbounded_channel();
    Arc::new(ClipManager::new(sender, directory.path().join("clips")))
}

fn register_segment(
    manager: &ClipManager,
    directory: &TempDir,
    name: &str,
    age: Duration,
) -> std::path::PathBuf {
    let path = directory.path().join(name);
    std::fs::write(&path, b"segment").expect("segment file should be written");
    let ended_at = std::time::SystemTime::now() - age;
    manager
        .register_segment(
            CAMERA_ID.to_owned(),
            path.clone(),
            ended_at - Duration::from_secs(2),
            ended_at,
        )
        .expect("segment should register");
    path
}
