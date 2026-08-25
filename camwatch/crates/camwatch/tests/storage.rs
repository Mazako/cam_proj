use tempfile::tempdir;

use camwatch::storage::{Database, NewCamera, NewEvent, NewUpload};

#[tokio::test]
async fn persists_an_event_and_upload_after_reopening_the_database() {
    let directory = tempdir().expect("temporary directory should be created");
    let database_path = directory.path().join("camwatch.sqlite3");

    let (database, was_created) = Database::open(&database_path)
        .await
        .expect("database should open");
    assert!(was_created);

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

    let camera = database
        .get_camera("front-door")
        .await
        .expect("camera should load")
        .expect("camera should exist");
    assert_eq!(camera.rtsp_url_env, "CAMWATCH_FRONT_DOOR_RTSP_URL");

    let event = database
        .create_event(NewEvent {
            camera_id: "front-door".to_owned(),
            started_at: 1_700_000_000_000,
            trigger: "motion".to_owned(),
        })
        .await
        .expect("event should be saved");
    let upload = database
        .create_upload(NewUpload {
            event_id: event.id.clone(),
            provider: "google_drive".to_owned(),
            next_attempt_at: Some(1_700_000_010_000),
        })
        .await
        .expect("upload should be saved");

    assert_eq!(
        database
            .camera_count()
            .await
            .expect("camera count should load"),
        1
    );
    assert_eq!(
        database
            .get_event(&event.id)
            .await
            .expect("event should load"),
        Some(event.clone())
    );
    assert_eq!(
        database
            .get_upload(&upload.id)
            .await
            .expect("upload should load"),
        Some(upload.clone())
    );

    drop(database);

    let (reopened, was_created) = Database::open(&database_path)
        .await
        .expect("existing database should reopen");
    assert!(!was_created);
    assert_eq!(
        reopened
            .camera_count()
            .await
            .expect("camera count should persist"),
        1
    );
    assert!(
        reopened
            .get_event(&event.id)
            .await
            .expect("event should persist")
            .is_some()
    );
    assert!(
        reopened
            .get_upload(&upload.id)
            .await
            .expect("upload should persist")
            .is_some()
    );
}
