use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use camwatch::{
    clips::{ClipJob, ClipManager},
    storage::{Database, NewCamera, NewSegment, Segment},
};
use tempfile::TempDir;
use tokio::sync::mpsc::{self, UnboundedReceiver, error::TryRecvError};

const CAMERA_ID: &str = "front-door";

#[tokio::test]
async fn keeps_pre_and_post_segments_reserved_until_the_clip_job_is_dropped() {
    let (directory, database) = database_with_camera().await;
    let (manager, mut receiver) = manager(&database, &directory);
    let pre_event = stored_segment(&database, "pre-event.mp4", 90, 95).await;
    let post_event = segment("post-event.mp4", 100, 110);

    manager
        .add_clip(
            CAMERA_ID.to_owned(),
            at(100),
            Duration::from_secs(10),
            Duration::from_secs(10),
        )
        .await
        .expect("clip should start");

    assert!(manager.is_camera_recording(CAMERA_ID));
    assert!(manager.is_segment_reserved(&pre_event.path));

    manager.put_segment_and_try_save_clip(post_event.clone());
    let job = receiver.try_recv().expect("clip job should be queued");

    assert!(!manager.is_camera_recording(CAMERA_ID));
    assert_eq!(job.segments, vec![pre_event.clone(), post_event.clone()]);
    assert!(manager.is_segment_reserved(&pre_event.path));
    assert!(manager.is_segment_reserved(&post_event.path));

    drop(job);

    assert!(!manager.is_segment_reserved(&pre_event.path));
    assert!(!manager.is_segment_reserved(&post_event.path));
}

#[tokio::test]
async fn keeps_a_shared_segment_reserved_until_every_clip_job_is_dropped() {
    let (directory, database) = database_with_camera().await;
    let (manager, mut receiver) = manager(&database, &directory);
    let shared_segment = stored_segment(&database, "shared.mp4", 100, 110).await;

    manager
        .add_clip(
            CAMERA_ID.to_owned(),
            at(105),
            Duration::from_secs(5),
            Duration::from_secs(5),
        )
        .await
        .expect("first clip should start");
    manager.put_segment_and_try_save_clip(shared_segment.clone());
    let first_job = receiver
        .try_recv()
        .expect("first clip job should be queued");

    manager
        .add_clip(
            CAMERA_ID.to_owned(),
            at(107),
            Duration::from_secs(7),
            Duration::from_secs(3),
        )
        .await
        .expect("second clip should start");
    manager.put_segment_and_try_save_clip(shared_segment.clone());
    let second_job = receiver
        .try_recv()
        .expect("second clip job should be queued");

    assert!(manager.is_segment_reserved(&shared_segment.path));

    drop(first_job);
    assert!(manager.is_segment_reserved(&shared_segment.path));

    drop(second_job);
    assert!(!manager.is_segment_reserved(&shared_segment.path));
}

#[tokio::test]
async fn queues_a_job_only_after_a_segment_reaches_the_post_event_boundary() {
    let (directory, database) = database_with_camera().await;
    let (manager, mut receiver) = manager(&database, &directory);

    manager
        .add_clip(
            CAMERA_ID.to_owned(),
            at(100),
            Duration::from_secs(10),
            Duration::from_secs(10),
        )
        .await
        .expect("clip should start");

    let insufficient_segment = segment("insufficient.mp4", 100, 109);
    manager.put_segment_and_try_save_clip(insufficient_segment.clone());

    assert!(manager.is_camera_recording(CAMERA_ID));
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
    assert!(manager.is_segment_reserved(&insufficient_segment.path));

    let final_segment = segment("final.mp4", 109, 110);
    manager.put_segment_and_try_save_clip(final_segment.clone());
    let job = receiver.try_recv().expect("clip job should be queued");

    assert_eq!(job.started_at, at(90));
    assert_eq!(job.ended_at, at(110));
    assert_eq!(job.segments, vec![insufficient_segment, final_segment]);
}

#[tokio::test]
async fn releases_segments_when_the_clip_worker_is_unavailable() {
    let (directory, database) = database_with_camera().await;
    let (manager, receiver) = manager(&database, &directory);
    drop(receiver);

    manager
        .add_clip(
            CAMERA_ID.to_owned(),
            at(100),
            Duration::ZERO,
            Duration::from_secs(10),
        )
        .await
        .expect("clip should start");

    let segment = segment("worker-unavailable.mp4", 100, 110);
    manager.put_segment_and_try_save_clip(segment.clone());

    assert!(!manager.is_camera_recording(CAMERA_ID));
    assert!(!manager.is_segment_reserved(&segment.path));
}

fn manager(
    database: &Database,
    directory: &TempDir,
) -> (Arc<ClipManager>, UnboundedReceiver<ClipJob>) {
    let (sender, receiver) = mpsc::unbounded_channel();
    let manager = Arc::new(ClipManager::new(
        database.clone(),
        sender,
        directory.path().join("clips"),
    ));

    (manager, receiver)
}

async fn database_with_camera() -> (TempDir, Database) {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let (database, _) = Database::open(&directory.path().join("camwatch.sqlite3"))
        .await
        .expect("database should open");
    database
        .upsert_cameras(&[NewCamera {
            id: CAMERA_ID.to_owned(),
            name: "Front door".to_owned(),
            rtsp_url: "CAMWATCH_FRONT_DOOR_RTSP_URL".to_owned(),
            onvif_url: None,
            onvif_credentials: None,
            motion_min_area: 1_000,
            yolo_confidence: 0.5,
            clip_after_motion: true,
        }])
        .await
        .expect("camera should be seeded");

    (directory, database)
}

async fn stored_segment(
    database: &Database,
    path: &str,
    started_at: u64,
    ended_at: u64,
) -> Segment {
    database
        .upsert_segment(NewSegment {
            camera_id: CAMERA_ID.to_owned(),
            path: path.to_owned(),
            started_at: millis(started_at),
            ended_at: millis(ended_at),
            size_bytes: 1,
        })
        .await
        .expect("segment should be stored")
}

fn segment(path: &str, started_at: u64, ended_at: u64) -> Segment {
    Segment {
        camera_id: CAMERA_ID.to_owned(),
        path: path.to_owned(),
        started_at: millis(started_at),
        ended_at: millis(ended_at),
        size_bytes: 1,
    }
}

fn at(seconds: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
}

fn millis(seconds: u64) -> i64 {
    i64::try_from(seconds * 1_000).expect("timestamp should fit in i64")
}
