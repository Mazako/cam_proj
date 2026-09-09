use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use camwatch::{
    clips::{ClipJob, ClipManager, create_clip_worker},
    config::{AppConfig, CameraConfig, Config},
    motion::{Mog2MotionDetector, MotionDetector},
    runtime::CameraRuntime,
    stream::{CameraStatusModel, CameraStream, CameraStreamEvent, GstreamerCameraStream},
};
use tempfile::tempdir;
use tokio::{
    sync::mpsc,
    time::{sleep, timeout},
};
use tokio_util::sync::CancellationToken;

use super::support::{
    RtspSession, assemble_pets2006_mp4, camera_stream, is_playable_mp4, pets2006_dataset,
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

#[tokio::test]
async fn saves_partial_clip_when_camera_goes_offline() {
    let directory = tempdir().expect("temporary directory should exist");
    let video_path = directory.path().join("pets2006.mp4");
    assemble_pets2006_mp4(&pets2006_dataset(), &video_path);
    let mut session = RtspSession::start("runtime-offline", Some(&video_path)).await;
    let app_config = app_config(60);
    let stream = camera_stream(session.url.clone(), &directory.path().join("segments"));
    let (upload_sender, mut upload_receiver) = mpsc::unbounded_channel();
    let clip_sender = create_clip_worker(upload_sender);
    let runtime = CameraRuntime::new(
        camera_config(true, 60),
        &app_config,
        stream,
        Arc::new(CameraStatusModel::default()),
        Arc::new(ClipManager::new(
            clip_sender,
            directory.path().join("clips"),
        )),
    )
    .await;
    let cancel = CancellationToken::new();
    let runtime_task = tokio::spawn(runtime.run(cancel.clone()));
    let mut observer = camera_stream(session.url.clone(), &directory.path().join("observer"));

    wait_for_motion(&mut observer, Duration::ZERO).await;
    sleep(Duration::from_secs(2)).await;
    session.publisher.stop();

    let upload = timeout(Duration::from_secs(20), upload_receiver.recv())
        .await
        .expect("offline camera should save its partial clip before timeout")
        .expect("clip worker should send an upload job");
    cancel.cancel();
    timeout(Duration::from_secs(5), runtime_task)
        .await
        .expect("runtime should stop after cancellation")
        .expect("runtime task should not panic");

    let clip = upload.request.clip;
    assert!(clip.path.is_file());
    assert!(clip.duration > Duration::ZERO);
    assert!(is_playable_mp4(&clip.path));
}

#[tokio::test]
async fn extends_clip_when_another_motion_is_detected() {
    let directory = tempdir().expect("temporary directory should exist");
    let video_path = directory.path().join("pets2006.mp4");
    assemble_pets2006_mp4(&pets2006_dataset(), &video_path);
    let mut session = RtspSession::start("runtime-extension", Some(&video_path)).await;
    let app_config = app_config(60);
    let stream = camera_stream(session.url.clone(), &directory.path().join("segments"));
    let (clip_sender, mut clip_receiver) = mpsc::unbounded_channel();
    let runtime = CameraRuntime::new(
        camera_config(true, 60),
        &app_config,
        stream,
        Arc::new(CameraStatusModel::default()),
        Arc::new(ClipManager::new(
            clip_sender,
            directory.path().join("clips"),
        )),
    )
    .await;
    let cancel = CancellationToken::new();
    let runtime_task = tokio::spawn(runtime.run(cancel.clone()));
    let mut observer = camera_stream(session.url.clone(), &directory.path().join("observer"));

    let first_motion = wait_for_motion(&mut observer, Duration::ZERO).await;
    let second_motion = wait_for_motion(&mut observer, Duration::from_secs(2)).await;
    sleep(Duration::from_secs(1)).await;
    session.publisher.stop();

    let clip = timeout(Duration::from_secs(20), clip_receiver.recv())
        .await
        .expect("offline camera should queue its extended clip before timeout")
        .expect("clip sender should remain connected");
    cancel.cancel();
    timeout(Duration::from_secs(5), runtime_task)
        .await
        .expect("runtime should stop after cancellation")
        .expect("runtime task should not panic");

    assert!(second_motion > first_motion + Duration::from_secs(2));
    assert!(clip.ended_at > first_motion + Duration::from_secs(61));
    assert!(clip.segments.len() > 1);
}

async fn run_runtime_until_clip(clip_after_motion: bool) -> ClipJob {
    let dataset = pets2006_dataset();
    let directory = tempdir().expect("temporary directory should exist");
    let video_path = directory.path().join("pets2006.mp4");
    assemble_pets2006_mp4(&dataset, &video_path);

    let session = RtspSession::start("runtime", Some(&video_path)).await;
    let stream = camera_stream(session.url.clone(), directory.path());
    let (clip_sender, mut clip_receiver) = tokio::sync::mpsc::unbounded_channel();
    let app_config = app_config(1);
    let clip_manager = Arc::new(ClipManager::new(
        clip_sender,
        app_config.clips_directory.clone(),
    ));
    let runtime = CameraRuntime::new(
        camera_config(clip_after_motion, 1),
        &app_config,
        stream,
        Arc::new(CameraStatusModel::default()),
        clip_manager,
    )
    .await;
    let cancel = CancellationToken::new();
    let runtime_task = tokio::spawn(runtime.run(cancel.clone()));

    let event = tokio::time::timeout(Duration::from_secs(60), clip_receiver.recv())
        .await
        .expect("runtime should queue a clip before timeout")
        .expect("clip sender should remain connected");
    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(1), runtime_task)
        .await
        .expect("runtime should stop after cancellation")
        .expect("runtime task should not panic");
    event
}

async fn wait_for_motion(
    stream: &mut GstreamerCameraStream,
    minimum_delay: Duration,
) -> SystemTime {
    let mut detector = Mog2MotionDetector::new().expect("motion detector should initialize");
    let mut first_motion = None;

    timeout(Duration::from_secs(45), async {
        loop {
            match stream
                .next_event()
                .await
                .expect("observer stream should stay available")
            {
                CameraStreamEvent::Frame(frame) => {
                    let captured_at = frame.captured_at;
                    let motion = detector
                        .detect(&frame)
                        .expect("motion detection should succeed");
                    if motion.largest_contour_area == 0.0 {
                        continue;
                    }
                    if let Some(first_motion) = first_motion {
                        if captured_at
                            .duration_since(first_motion)
                            .is_ok_and(|delay| delay >= minimum_delay)
                        {
                            return captured_at;
                        }
                    } else {
                        first_motion = Some(captured_at);
                        if minimum_delay.is_zero() {
                            return captured_at;
                        }
                    }
                }
                CameraStreamEvent::Status(_) | CameraStreamEvent::SegmentFinalized { .. } => {}
            }
        }
    })
    .await
    .expect("PETS2006 stream should contain motion")
}

fn camera_config(clip_after_motion: bool, post_event_seconds: u32) -> CameraConfig {
    Config::parse(&format!(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "data/camwatch.sqlite3"
pre_event_seconds = 1
post_event_seconds = {post_event_seconds}
rolling_buffer_seconds = 120
segment_rotation_seconds = 1

[[cameras]]
id = "front-door"
name = "Front door"
rtsp_url = "rtsp://127.0.0.1:8554/front-door"
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

fn app_config(post_event_seconds: u32) -> AppConfig {
    Config::parse(&format!(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "data/camwatch.sqlite3"
pre_event_seconds = 1
post_event_seconds = {post_event_seconds}
rolling_buffer_seconds = 120
segment_rotation_seconds = 1
"#,
    ))
    .expect("app configuration should parse")
    .app
}
