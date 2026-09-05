use std::{path::Path, sync::Arc, time::SystemTime};

use camwatch::{
    config::{Config, SecretManager},
    stream::CameraStreamStatus,
};
use camwatch_server::{
    app_state::{AppState, bootstrap_with_secret_manager},
    error::ServerStartupError,
};

fn test_secret_manager() -> Arc<SecretManager> {
    Arc::new(SecretManager::from_key([9; 32]))
}

async fn bootstrap(config: Config) -> Result<AppState, ServerStartupError> {
    bootstrap_with_secret_manager(config, test_secret_manager()).await
}

#[tokio::test]
async fn bootstraps_directly_in_the_server_without_external_integrations() {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let database_path = directory.path().join("camwatch.sqlite3");
    let state = bootstrap(config(&database_path, false))
        .await
        .expect("server should bootstrap without a reachable camera");

    assert_eq!(
        state
            .database
            .camera_count()
            .await
            .expect("camera count should be available"),
        1
    );
    assert!(database_path.exists());

    let summaries = state
        .camera_summaries()
        .await
        .expect("camera summaries should load");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "front-door");
    assert_eq!(summaries[0].name, "Front door");
    state.status_model.update(
        "front-door",
        CameraStreamStatus::Online {
            since: SystemTime::UNIX_EPOCH,
        },
    );
    assert_eq!(
        state
            .camera_summaries()
            .await
            .expect("camera summaries should load")[0]
            .stream_status,
        Some(CameraStreamStatus::Online {
            since: SystemTime::UNIX_EPOCH,
        })
    );

    let details = state
        .camera_details("front-door")
        .await
        .expect("camera details should load")
        .expect("camera details should exist");
    assert_eq!(details.summary.id, "front-door");
    assert_eq!(details.rtsp_codec, "h264");
    assert!(details.clip_after_motion);
    assert!(
        state
            .camera_details("missing")
            .await
            .expect("missing camera lookup should succeed")
            .is_none()
    );
}

#[tokio::test]
async fn upserts_toml_cameras_and_reloads_them_from_database() {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let database_path = directory.path().join("camwatch.sqlite3");

    bootstrap(config_with_camera(
        &database_path,
        false,
        "Front door",
        "h264",
        true,
    ))
    .await
    .expect("initial bootstrap should succeed");

    let updated_state = bootstrap(config_with_camera(
        &database_path,
        false,
        "Updated front door",
        "h265",
        false,
    ))
    .await
    .expect("updated bootstrap should succeed");
    let camera = updated_state
        .database
        .get_camera("front-door")
        .await
        .expect("camera should load")
        .expect("camera should exist");
    assert_eq!(camera.name, "Updated front door");
    assert_eq!(camera.rtsp_codec, "h265");
    assert!(!camera.clip_after_motion);

    let restarted_state = bootstrap(config_without_cameras(&database_path))
        .await
        .expect("restart should load cameras from the database");
    assert_eq!(restarted_state.database.camera_count().await.unwrap(), 1);
}

#[tokio::test]
async fn returns_a_controlled_error_for_invalid_enabled_r2() {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let database_path = directory.path().join("camwatch.sqlite3");

    let mut config = config(&database_path, true);
    config.app.r2_endpoint = None;

    assert!(matches!(
        bootstrap(config).await,
        Err(ServerStartupError::Configuration(_))
    ));
}

fn config(database_path: &Path, r2_enabled: bool) -> Config {
    config_with_camera(database_path, r2_enabled, "Front door", "h264", true)
}

fn config_with_camera(
    database_path: &Path,
    r2_enabled: bool,
    name: &str,
    rtsp_codec: &str,
    clip_after_motion: bool,
) -> Config {
    let r2 = if r2_enabled {
        format!(
            r#"
r2_enabled = true
r2_endpoint = "{}"
r2_access_key_id = "{}"
r2_secret_access_key = "{}"
r2_bucket = "{}"
"#,
            SecretManager::from_key([9; 32])
                .encrypt("https://r2.example.com")
                .expect("test secret should encrypt"),
            SecretManager::from_key([9; 32])
                .encrypt("access-key")
                .expect("test secret should encrypt"),
            SecretManager::from_key([9; 32])
                .encrypt("secret-key")
                .expect("test secret should encrypt"),
            SecretManager::from_key([9; 32])
                .encrypt("bucket")
                .expect("test secret should encrypt"),
        )
    } else {
        String::new()
    };
    let contents = format!(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "{}"
pre_event_seconds = 10
post_event_seconds = 20
rolling_buffer_seconds = 30
{}

[[cameras]]
id = "front-door"
name = "Front door"
rtsp_url = "{}"
rtsp_codec = "h264"
motion_min_area = 1000
yolo_confidence = 0.5
clip_after_motion = true
        "#,
        database_path.display(),
        r2,
        SecretManager::from_key([9; 32])
            .encrypt("rtsp://camera.local/front-door")
            .expect("test secret should encrypt"),
    )
    .replace("name = \"Front door\"", &format!("name = \"{name}\""))
    .replace(
        "rtsp_codec = \"h264\"",
        &format!("rtsp_codec = \"{rtsp_codec}\""),
    )
    .replace(
        "clip_after_motion = true",
        &format!("clip_after_motion = {clip_after_motion}"),
    );

    Config::parse(&contents).expect("test configuration should parse")
}

fn config_without_cameras(database_path: &Path) -> Config {
    let contents = format!(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "{}"
pre_event_seconds = 10
post_event_seconds = 20
rolling_buffer_seconds = 30
"#,
        database_path.display()
    );

    Config::parse(&contents).expect("test configuration should parse")
}
