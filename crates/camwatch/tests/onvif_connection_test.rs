use camwatch::{
    config::Config,
    onvif::{OnvifConnection, PtzDirection},
};
use oxvif::mock::MockServer;

#[tokio::test]
async fn builds_against_a_mock_camera_with_ptz() {
    let server = MockServer::start().await.expect("mock server should start");
    let connection = OnvifConnection::try_build(&camera_config(server.device_url()))
        .await
        .expect("connection should build");

    connection
        .cam_move(PtzDirection::Right(0.5))
        .await
        .expect("ptz move should succeed");
}

#[tokio::test]
async fn returns_none_when_onvif_url_is_missing() {
    let config = Config::parse(
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
rtsp_url = "rtsp://127.0.0.1:8554/front-door"
motion_min_area = 1000
yolo_confidence = 0.5
"#,
    )
    .expect("configuration should parse");
    let camera = config.cameras.into_iter().next().expect("camera");

    assert!(OnvifConnection::try_build(&camera).await.is_none());
}

#[tokio::test]
async fn returns_none_when_onvif_device_is_unreachable() {
    let camera = camera_config("http://127.0.0.1:9/onvif/device");

    assert!(OnvifConnection::try_build(&camera).await.is_none());
}

fn camera_config(onvif_url: &str) -> camwatch::config::CameraConfig {
    Config::parse(&format!(
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
rtsp_url = "rtsp://127.0.0.1:8554/front-door"
onvif_url = "{onvif_url}"
onvif_credentials = "user:password"
motion_min_area = 1000
yolo_confidence = 0.5
"#
    ))
    .expect("camera configuration should parse")
    .cameras
    .into_iter()
    .next()
    .expect("configuration should contain a camera")
}
