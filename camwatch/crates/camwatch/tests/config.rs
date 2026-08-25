use camwatch::config::Config;

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
onvif_url = "http://192.168.1.65:2020/onvif/device_service"
onvif_credentials_env = "CAMWATCH_FRONT_DOOR_ONVIF_CREDENTIALS"
motion_min_area = 1000
yolo_confidence = 0.5
"#;

#[test]
fn parses_a_configuration_with_a_camera() {
    let config = Config::parse(VALID_CONFIG).expect("valid configuration should load");

    assert_eq!(config.cameras.len(), 1);
    assert_eq!(config.app.bind_address.to_string(), "127.0.0.1:8080");
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
