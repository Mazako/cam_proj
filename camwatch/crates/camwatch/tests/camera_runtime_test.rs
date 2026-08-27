use std::{collections::VecDeque, sync::Arc, time::SystemTime};

use camwatch::{
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

    CameraRuntime::new(
        "front-door".to_owned(),
        stream,
        Arc::clone(&status_model),
        database,
    )
    .run()
    .await;

    assert_eq!(
        status_model.get("front-door"),
        Some(CameraStreamStatus::Online {
            since: SystemTime::UNIX_EPOCH,
        })
    );
}
