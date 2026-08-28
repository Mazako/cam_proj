use std::{collections::VecDeque, sync::Arc, time::SystemTime};

use camwatch::{
    config::{AppConfig, Config},
    ports::{CameraStream, CameraStreamError, CameraStreamEvent, CameraStreamStatus, PortFuture},
    runtime::CameraRuntime,
    storage::Database,
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

    let (clip_sender, _clip_receiver) = tokio::sync::mpsc::unbounded_channel();
    let app_config = app_config();
    let runtime = CameraRuntime::new(
        camera_config(),
        &app_config,
        stream,
        Arc::clone(&status_model),
        database,
        clip_sender,
    );
    assert_eq!(runtime.pre_event_seconds, 10);
    assert_eq!(runtime.post_event_seconds, 20);
    assert_eq!(runtime.clips_directory.to_string_lossy(), "data/clips");
    runtime.run().await;

    assert_eq!(
        status_model.get("front-door"),
        Some(CameraStreamStatus::Online {
            since: SystemTime::UNIX_EPOCH,
        })
    );
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
