use tempfile::tempdir;

use camwatch::storage::{Database, EventStatus, NewCamera, NewEvent, NewUpload, UploadStatus};

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

#[tokio::test]
async fn loads_all_status_values_directly_into_domain_models() {
    let directory = tempdir().expect("temporary directory should be created");
    let database_path = directory.path().join("camwatch.sqlite3");
    let (database, _) = Database::open(&database_path)
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

    let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", database_path.display()))
        .await
        .expect("test connection should open");

    let event_statuses = [
        ("recording", EventStatus::Recording),
        ("finalizing", EventStatus::Finalizing),
        ("ready", EventStatus::Ready),
        ("failed", EventStatus::Failed),
    ];
    let upload_statuses = [
        ("pending", UploadStatus::Pending),
        ("in_progress", UploadStatus::InProgress),
        ("uploaded", UploadStatus::Uploaded),
        ("failed", UploadStatus::Failed),
    ];

    for (index, (stored_status, expected_status)) in event_statuses.iter().enumerate() {
        let event_id = format!("event-{index}");
        sqlx::query(
            "INSERT INTO \"events\" (
                id, camera_id, started_at, \"trigger\", status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&event_id)
        .bind("front-door")
        .bind(1_700_000_000_000_i64)
        .bind("motion")
        .bind(*stored_status)
        .bind(1_700_000_000_000_i64)
        .bind(1_700_000_000_000_i64)
        .execute(&pool)
        .await
        .expect("event status should be inserted");

        let event = database
            .get_event(&event_id)
            .await
            .expect("event should load")
            .expect("event should exist");
        assert_eq!(event.status, *expected_status);
    }

    for (index, (stored_status, expected_status)) in upload_statuses.iter().enumerate() {
        let upload_id = format!("upload-{index}");
        sqlx::query(
            "INSERT INTO uploads (
                id, event_id, provider, status, attempt_count, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&upload_id)
        .bind("event-0")
        .bind("google_drive")
        .bind(*stored_status)
        .bind(0_i64)
        .bind(1_700_000_000_000_i64)
        .bind(1_700_000_000_000_i64)
        .execute(&pool)
        .await
        .expect("upload status should be inserted");

        let upload = database
            .get_upload(&upload_id)
            .await
            .expect("upload should load")
            .expect("upload should exist");
        assert_eq!(upload.status, *expected_status);
    }
}
