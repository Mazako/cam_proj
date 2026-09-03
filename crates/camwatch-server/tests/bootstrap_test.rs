use std::path::Path;

use camwatch::config::Config;
use camwatch_server::{app_state::bootstrap, error::ServerStartupError};

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

    assert!(!state.runtime_running("front-door"));
    assert!(!state.ptz_available("front-door"));
}

#[tokio::test]
async fn returns_a_controlled_error_for_invalid_enabled_r2() {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let database_path = directory.path().join("camwatch.sqlite3");

    assert!(matches!(
        bootstrap(config(&database_path, true)).await,
        Err(ServerStartupError::R2Configuration(_))
    ));
}

fn config(database_path: &Path, r2_enabled: bool) -> Config {
    let r2 = if r2_enabled {
        r#"
r2_enabled = true
r2_endpoint_env = "CAMWATCH_WEB02_TEST_R2_ENDPOINT"
r2_access_key_id_env = "CAMWATCH_WEB02_TEST_R2_ACCESS_KEY"
r2_secret_access_key_env = "CAMWATCH_WEB02_TEST_R2_SECRET"
r2_bucket_env = "CAMWATCH_WEB02_TEST_R2_BUCKET"
"#
    } else {
        ""
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
rtsp_url_env = "CAMWATCH_WEB02_TEST_RTSP_URL"
motion_min_area = 1000
yolo_confidence = 0.5
"#,
        database_path.display(),
        r2
    );

    Config::parse(&contents).expect("test configuration should parse")
}
