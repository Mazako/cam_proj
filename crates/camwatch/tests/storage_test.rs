use tempfile::tempdir;

use camwatch::storage::{Database, NewCamera, NewSegment};

#[tokio::test]
async fn persists_cameras_and_segments_after_reopening_the_database() {
    let directory = tempdir().expect("temporary directory should be created");
    let database_path = directory.path().join("camwatch.sqlite3");

    let (database, was_created) = Database::open(&database_path)
        .await
        .expect("database should open");
    assert!(was_created);

    database
        .upsert_cameras(&[NewCamera {
            id: "front-door".to_owned(),
            name: "Front door".to_owned(),
            rtsp_url: "CAMWATCH_FRONT_DOOR_RTSP_URL".to_owned(),
            onvif_url: None,
            onvif_credentials: None,
            motion_min_area: 1000,
            yolo_confidence: 0.5,
            clip_after_motion: true,
        }])
        .await
        .expect("camera should be seeded");

    database
        .upsert_segment(NewSegment {
            camera_id: "front-door".to_owned(),
            path: "/data/segments/front-door/segment-0000000000.mp4".to_owned(),
            started_at: 1_700_000_000_000,
            ended_at: 1_700_000_002_000,
            size_bytes: 42,
        })
        .await
        .expect("segment should be saved");

    let schema_pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", database_path.display()))
        .await
        .expect("schema connection should open");
    let historical_tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master
         WHERE type = 'table' AND name IN ('events', 'uploads')
         ORDER BY name",
    )
    .fetch_all(&schema_pool)
    .await
    .expect("schema should be readable");
    assert!(historical_tables.is_empty());
    schema_pool.close().await;

    assert_eq!(
        database
            .camera_count()
            .await
            .expect("camera count should load"),
        1
    );
    assert_eq!(
        database
            .get_camera("front-door")
            .await
            .expect("camera should load")
            .expect("camera should exist")
            .rtsp_url,
        "CAMWATCH_FRONT_DOOR_RTSP_URL"
    );
    let camera = database
        .get_camera("front-door")
        .await
        .expect("camera should load")
        .expect("camera should exist");
    assert!(camera.clip_after_motion);
    assert_eq!(
        database
            .segments_overlapping("front-door", 1_700_000_001_000, 1_700_000_001_000)
            .await
            .expect("segment should load")
            .len(),
        1
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
    assert_eq!(
        reopened
            .segments_overlapping("front-door", 1_700_000_001_000, 1_700_000_001_000)
            .await
            .expect("segment should survive reopening")
            .len(),
        1
    );
}
