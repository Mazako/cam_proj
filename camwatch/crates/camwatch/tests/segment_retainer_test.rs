use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use camwatch::{
    clips::{ClipManager, retain_expired_segments},
    storage::{Database, NewCamera, NewSegment, Segment},
};
use tempfile::TempDir;
use tokio::{fs, sync::mpsc};

const CAMERA_ID: &str = "front-door";
const ROLLING_BUFFER_SECONDS: u64 = 30;

#[tokio::test]
async fn removes_expired_unreserved_segments() {
    let (directory, database, manager) = setup().await;
    let expired = store_segment_file(&directory, &database, "expired.mp4", 90).await;
    let fresh = store_segment_file(&directory, &database, "fresh.mp4", 5).await;

    retain_expired_segments(&database, ROLLING_BUFFER_SECONDS, &manager).await;

    assert!(!PathBuf::from(&expired.path).exists());
    assert!(PathBuf::from(&fresh.path).exists());
    assert!(!segment_exists(&database, &expired.path).await);
    assert!(segment_exists(&database, &fresh.path).await);
}

#[tokio::test]
async fn keeps_reserved_segments() {
    let (directory, database, manager) = setup().await;
    let reserved = store_segment_file(&directory, &database, "reserved.mp4", 90).await;

    manager
        .add_clip(
            CAMERA_ID.to_owned(),
            SystemTime::now(),
            Duration::from_secs(120),
            Duration::from_secs(10),
        )
        .await
        .expect("clip should start");

    let unreserved = store_segment_file(&directory, &database, "unreserved.mp4", 90).await;

    assert!(manager.is_segment_reserved(&reserved.path));
    assert!(!manager.is_segment_reserved(&unreserved.path));

    retain_expired_segments(&database, ROLLING_BUFFER_SECONDS, &manager).await;

    assert!(PathBuf::from(&reserved.path).exists());
    assert!(segment_exists(&database, &reserved.path).await);
    assert!(!PathBuf::from(&unreserved.path).exists());
    assert!(!segment_exists(&database, &unreserved.path).await);
}

#[tokio::test]
async fn removes_database_rows_when_segment_files_are_already_gone() {
    let (_directory, database, manager) = setup().await;
    let orphan = store_segment(
        &database,
        "/tmp/camwatch-missing-segment-that-does-not-exist.mp4",
        90,
    )
    .await;

    assert!(!PathBuf::from(&orphan.path).exists());

    retain_expired_segments(&database, ROLLING_BUFFER_SECONDS, &manager).await;

    assert!(!segment_exists(&database, &orphan.path).await);
}

async fn setup() -> (TempDir, Database, Arc<ClipManager>) {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let (database, _) = Database::open(&directory.path().join("camwatch.sqlite3"))
        .await
        .expect("database should open");
    database
        .seed_cameras(&[NewCamera {
            id: CAMERA_ID.to_owned(),
            name: "Front door".to_owned(),
            rtsp_url_env: "CAMWATCH_FRONT_DOOR_RTSP_URL".to_owned(),
            onvif_url: None,
            onvif_credentials_env: None,
            motion_min_area: 1_000,
            yolo_confidence: 0.5,
        }])
        .await
        .expect("camera should be seeded");

    let (sender, _receiver) = mpsc::unbounded_channel();
    let manager = Arc::new(ClipManager::new(
        database.clone(),
        sender,
        directory.path().join("clips"),
    ));

    (directory, database, manager)
}

async fn store_segment_file(
    directory: &TempDir,
    database: &Database,
    name: &str,
    ended_secs_ago: u64,
) -> Segment {
    let path = directory.path().join(name);
    fs::write(&path, b"segment")
        .await
        .expect("segment file should be written");
    store_segment(database, path.to_str().expect("utf-8 path"), ended_secs_ago).await
}

async fn store_segment(database: &Database, path: &str, ended_secs_ago: u64) -> Segment {
    let ended_at = millis_ago(ended_secs_ago);
    database
        .upsert_segment(NewSegment {
            camera_id: CAMERA_ID.to_owned(),
            path: path.to_owned(),
            started_at: ended_at - 2_000,
            ended_at,
            size_bytes: 1,
        })
        .await
        .expect("segment should be stored")
}

async fn segment_exists(database: &Database, path: &str) -> bool {
    database
        .segments_overlapping(CAMERA_ID, 0, i64::MAX)
        .await
        .expect("segments should load")
        .into_iter()
        .any(|segment| segment.path == path)
}

fn millis_ago(seconds: u64) -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_millis();
    i64::try_from(now - u128::from(seconds) * 1_000).expect("timestamp should fit in i64")
}
