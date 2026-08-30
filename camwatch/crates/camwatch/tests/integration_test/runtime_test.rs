use std::{sync::Arc, time::Duration};

use camwatch::{
    clips::{ClipJob, ClipManager},
    config::{AppConfig, CameraConfig, Config},
    runtime::CameraRuntime,
    stream::CameraStatusModel,
};
use tempfile::tempdir;

use super::support::{
    RtspSession, assemble_pets2006_mp4, camera_stream, database_with_camera, pets2006_dataset,
};

#[tokio::test]
async fn queues_clip_when_motion_is_sufficient() {
    let event = run_runtime_until_clip(true).await;

    assert!(event.path.to_string_lossy().ends_with(".mp4"));
    assert!(event.ended_at > event.started_at);
}

#[tokio::test]
async fn queues_clip_when_motion_and_yolo_are_required() {
    let event = run_runtime_until_clip(false).await;

    assert!(event.path.to_string_lossy().ends_with(".mp4"));
    assert!(event.ended_at > event.started_at);
}

async fn run_runtime_until_clip(clip_after_motion: bool) -> ClipJob {
    let dataset = pets2006_dataset();
    let directory = tempdir().expect("temporary directory should exist");
    let video_path = directory.path().join("pets2006.mp4");
    assemble_pets2006_mp4(&dataset, &video_path);

    let database = database_with_camera(directory.path()).await;
    let session = RtspSession::start("runtime", Some(&video_path)).await;
    let stream = camera_stream(session.url.clone(), directory.path());
    let (clip_sender, mut clip_receiver) = tokio::sync::mpsc::unbounded_channel();
    let app_config = app_config();
    let clip_manager = Arc::new(ClipManager::new(
        database.clone(),
        clip_sender,
        app_config.clips_directory.clone(),
    ));
    let runtime = CameraRuntime::new(
        camera_config(clip_after_motion),
        &app_config,
        stream,
        Arc::new(CameraStatusModel::default()),
        database,
        clip_manager,
    );
    let runtime_task = tokio::spawn(runtime.run());

    let event = tokio::time::timeout(Duration::from_secs(60), clip_receiver.recv())
        .await
        .expect("runtime should queue a clip before timeout")
        .expect("clip sender should remain connected");
    runtime_task.abort();
    event
}

fn camera_config(clip_after_motion: bool) -> CameraConfig {
    Config::parse(&format!(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "data/camwatch.sqlite3"
pre_event_seconds = 1
post_event_seconds = 1
rolling_buffer_seconds = 30
segment_rotation_seconds = 1

[[cameras]]
id = "front-door"
name = "Front door"
rtsp_url_env = "CAMWATCH_FRONT_DOOR_RTSP_URL"
motion_min_area = 1000
yolo_confidence = 0.3
clip_after_motion = {clip_after_motion}
"#
    ))
    .expect("camera configuration should parse")
    .cameras
    .into_iter()
    .next()
    .expect("configuration should contain a camera")
}

fn app_config() -> AppConfig {
    Config::parse(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "data/camwatch.sqlite3"
pre_event_seconds = 1
post_event_seconds = 1
rolling_buffer_seconds = 30
segment_rotation_seconds = 1
"#,
    )
    .expect("app configuration should parse")
    .app
}
