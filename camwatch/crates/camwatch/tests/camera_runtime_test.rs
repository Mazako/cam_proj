use std::{
    collections::VecDeque,
    fs,
    sync::Arc,
    time::{Duration, SystemTime},
};

use camwatch::{
    clips::ClipManager,
    config::{AppConfig, Config},
    ports::{
        CameraStream, CameraStreamError, CameraStreamEvent, CameraStreamStatus, Frame, PixelFormat,
        PortFuture,
    },
    runtime::CameraRuntime,
    storage::{Database, NewCamera},
    stream::CameraStatusModel,
};
use tempfile::tempdir;

struct FakeCameraStream {
    events: VecDeque<Result<CameraStreamEvent, CameraStreamError>>,
}

impl CameraStream for FakeCameraStream {
    fn next_event(&mut self) -> PortFuture<'_, Result<CameraStreamEvent, CameraStreamError>> {
        let event = self
            .events
            .pop_front()
            .unwrap_or(Err(CameraStreamError::Unavailable));
        Box::pin(async move { event })
    }
}

#[tokio::test]
async fn updates_status_from_the_camera_stream() {
    let directory = tempdir().expect("temporary directory should exist");
    let (database, _) = Database::open(&directory.path().join("camwatch.sqlite3"))
        .await
        .expect("database should open");
    let status_model = Arc::new(CameraStatusModel::default());
    let stream = FakeCameraStream {
        events: VecDeque::from([
            Ok(CameraStreamEvent::Status(CameraStreamStatus::Online {
                since: SystemTime::UNIX_EPOCH,
            })),
            Err(CameraStreamError::Unavailable),
        ]),
    };

    let app_config = app_config();
    let (clip_sender, _clip_receiver) = tokio::sync::mpsc::unbounded_channel();
    let clip_manager = Arc::new(ClipManager::new(
        database.clone(),
        clip_sender,
        app_config.clips_directory.clone(),
    ));
    let runtime = CameraRuntime::new(
        camera_config(),
        &app_config,
        stream,
        Arc::clone(&status_model),
        database,
        clip_manager,
    )
    .await;
    assert_eq!(runtime.pre_event_seconds, 10);
    assert_eq!(runtime.post_event_seconds, 20);
    runtime.run().await;

    assert_eq!(
        status_model.get("front-door"),
        Some(CameraStreamStatus::Online {
            since: SystemTime::UNIX_EPOCH,
        })
    );
}

#[tokio::test]
async fn queues_clip_with_pre_and_post_window() {
    let directory = tempdir().expect("temporary directory should exist");
    let (database, _) = Database::open(&directory.path().join("camwatch.sqlite3"))
        .await
        .expect("database should open");
    database
        .seed_cameras(&[NewCamera {
            id: "front-door".to_owned(),
            name: "Front door".to_owned(),
            rtsp_url_env: "CAMWATCH_FRONT_DOOR_RTSP_URL".to_owned(),
            onvif_url: None,
            onvif_credentials_env: None,
            motion_min_area: 1000,
            yolo_confidence: 0.5,
        }])
        .await
        .expect("camera should be seeded");
    let detected_at = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let segment_path = directory.path().join("segment.mp4");
    fs::write(&segment_path, b"segment").expect("segment should be created");

    let width = 640;
    let height = 480;
    let black_frame = vec![0; width * height * 3];
    let mut white_frame = black_frame.clone();
    for pixel in white_frame
        .chunks_exact_mut(3)
        .take((width / 2) * (height / 2))
    {
        pixel.fill(255);
    }
    let mut events = VecDeque::new();
    for index in 0..90 {
        events.push_back(Ok(CameraStreamEvent::Frame(Frame::new(
            black_frame.clone(),
            width as u32,
            height as u32,
            PixelFormat::Bgr8,
            detected_at + Duration::from_secs(index),
        ))));
    }
    events.push_back(Ok(CameraStreamEvent::Frame(Frame::new(
        white_frame,
        width as u32,
        height as u32,
        PixelFormat::Bgr8,
        detected_at + Duration::from_secs(90),
    ))));
    events.push_back(Ok(CameraStreamEvent::SegmentFinalized {
        path: segment_path,
        started_at: detected_at + Duration::from_secs(80),
        ended_at: detected_at + Duration::from_secs(111),
    }));
    events.push_back(Err(CameraStreamError::Unavailable));

    let stream = FakeCameraStream { events };
    let (clip_sender, mut clip_receiver) = tokio::sync::mpsc::unbounded_channel();
    let clip_manager = Arc::new(ClipManager::new(
        database.clone(),
        clip_sender,
        app_config().clips_directory,
    ));
    let runtime = CameraRuntime::new(
        camera_config(),
        &app_config(),
        stream,
        Arc::new(CameraStatusModel::default()),
        database,
        Arc::clone(&clip_manager),
    )
    .await;
    runtime.run().await;

    let clip = clip_receiver
        .recv()
        .await
        .expect("runtime should queue a clip");
    let motion_detected_at = detected_at + Duration::from_secs(90);
    assert_eq!(
        clip.started_at,
        motion_detected_at - Duration::from_secs(10)
    );
    assert_eq!(clip.ended_at, motion_detected_at + Duration::from_secs(20));
    assert!(clip.path.to_string_lossy().ends_with(".mp4"));
    assert_eq!(clip.segments.len(), 1);

    let reserved_path = clip.segments[0].path.clone();
    assert!(clip_manager.is_segment_reserved(&reserved_path));
    drop(clip);
    assert!(!clip_manager.is_segment_reserved(&reserved_path));
}

fn camera_config() -> camwatch::config::CameraConfig {
    Config::parse(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "data/camwatch.sqlite3"
pre_event_seconds = 10
post_event_seconds = 20
rolling_buffer_seconds = 30

[[cameras]]
id = "front-door"
name = "Front door"
rtsp_url_env = "CAMWATCH_FRONT_DOOR_RTSP_URL"
motion_min_area = 1000
yolo_confidence = 0.5
"#,
    )
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
pre_event_seconds = 10
post_event_seconds = 20
rolling_buffer_seconds = 30
"#,
    )
    .expect("app configuration should parse")
    .app
}
