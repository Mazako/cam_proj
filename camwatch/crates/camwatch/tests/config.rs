use camwatch::{config::Config, stream::RtspCodec};

const VALID_CONFIG: &str = r#"
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
rtsp_codec = "h265"
onvif_url = "http://192.168.1.65:2020/onvif/device_service"
onvif_credentials_env = "CAMWATCH_FRONT_DOOR_ONVIF_CREDENTIALS"
motion_min_area = 1000
yolo_confidence = 0.5
"#;

#[test]
fn parses_a_configuration_with_a_camera() {
    let config = Config::parse(VALID_CONFIG).expect("valid configuration should load");

    assert_eq!(config.cameras.len(), 1);
    assert_eq!(config.cameras[0].rtsp_codec, RtspCodec::H265);
    assert_eq!(config.app.bind_address.to_string(), "127.0.0.1:8080");
    assert_eq!(
        config.app.segment_directory.to_string_lossy(),
        "data/segments"
    );
    assert_eq!(config.app.segment_rotation_seconds, 2);
}

#[test]
fn defaults_camera_codec_to_h264() {
    let input = VALID_CONFIG.replace("rtsp_codec = \"h265\"\n", "");

    let config = Config::parse(&input).expect("configuration should default to H.264");

    assert_eq!(config.cameras[0].rtsp_codec, RtspCodec::H264);
}

#[test]
fn parses_a_configuration_without_bootstrap_cameras() {
    let config = Config::parse(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "data/camwatch.sqlite3"
pre_event_seconds = 10
post_event_seconds = 20
rolling_buffer_seconds = 30
"#,
    )
    .expect("configuration without bootstrap cameras should load");

    assert!(config.cameras.is_empty());
}

#[test]
fn rejects_an_invalid_environment_variable_name_without_disclosing_it() {
    let input = VALID_CONFIG.replace(
        "CAMWATCH_FRONT_DOOR_RTSP_URL",
        "rtsp://user:super-secret-rtsp-url@camera.local/live",
    );

    let error =
        Config::parse(&input).expect_err("a secret value cannot be an environment variable name");

    assert!(!error.to_string().contains("super-secret-rtsp-url"));
}

#[test]
fn rejects_incomplete_onvif_configuration() {
    let input = VALID_CONFIG.replace(
        "onvif_credentials_env = \"CAMWATCH_FRONT_DOOR_ONVIF_CREDENTIALS\"\n",
        "",
    );

    let error = Config::parse(&input).expect_err("ONVIF requires both fields");

    assert_eq!(
        error.to_string(),
        "invalid configuration: onvif_url and onvif_credentials_env must be set together"
    );
}

#[test]
fn rejects_a_segment_rotation_longer_than_the_rolling_buffer() {
    let input = VALID_CONFIG.replace(
        "rolling_buffer_seconds = 30",
        "rolling_buffer_seconds = 10\nsegment_rotation_seconds = 11",
    );

    let error = Config::parse(&input).expect_err("rotation cannot exceed the rolling buffer");

    assert_eq!(
        error.to_string(),
        "invalid configuration: segment_rotation_seconds cannot exceed rolling_buffer_seconds"
    );
}
