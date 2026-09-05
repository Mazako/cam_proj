use camwatch::config::{Config, SecretManager};

const TEST_KEY: [u8; 32] = [9; 32];

fn secret(value: &str) -> String {
    SecretManager::from_key(TEST_KEY)
        .encrypt(value)
        .expect("test secret should encrypt")
}

fn valid_config() -> String {
    config_with_values(
        "rtsp://camera.local/live",
        Some("user:password"),
        "http://192.168.1.65:2020/onvif/device_service",
    )
}

fn config_with_values(rtsp_url: &str, credentials: Option<&str>, onvif_url: &str) -> String {
    let credentials = credentials
        .map(secret)
        .map(|value| format!("onvif_credentials = \"{value}\"\n"))
        .unwrap_or_default();
    format!(
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
rtsp_url = "{}"
onvif_url = "{}"
{}motion_min_area = 1000
yolo_confidence = 0.5
clip_after_motion = false
"#,
        secret(rtsp_url),
        onvif_url,
        credentials,
    )
}

fn parse_config(contents: &str) -> Config {
    Config::parse(contents)
        .expect("configuration should parse")
        .decrypt_secrets(&SecretManager::from_key(TEST_KEY))
        .expect("configuration secrets should decrypt")
}

#[test]
fn parses_a_configuration_with_a_camera() {
    let config = parse_config(&valid_config());

    assert_eq!(config.cameras.len(), 1);
    assert_eq!(config.app.bind_address.to_string(), "127.0.0.1:8080");
    assert_eq!(
        config.app.segment_directory.to_string_lossy(),
        "data/segments"
    );
    assert_eq!(config.app.clips_directory.to_string_lossy(), "data/clips");
    assert_eq!(config.app.segment_rotation_seconds, 2);
    assert!(!config.app.r2_enabled);
    assert!(!config.cameras[0].clip_after_motion);
}

#[test]
fn defaults_clip_after_motion_to_true() {
    let input = valid_config().replace("clip_after_motion = false\n", "");

    let config = parse_config(&input);

    assert!(config.cameras[0].clip_after_motion);
}

#[test]
fn does_not_read_r2_secrets_when_r2_is_disabled() {
    let input = valid_config().replace(
        "rolling_buffer_seconds = 30",
        "rolling_buffer_seconds = 30\nr2_endpoint = \"not-a-ciphertext\"",
    );

    let config = parse_config(&input);

    assert_eq!(config.app.r2_endpoint.as_deref(), Some("not-a-ciphertext"));
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
fn rejects_an_invalid_rtsp_url_without_disclosing_it() {
    let input = config_with_values(
        "http://user:super-secret-rtsp-url@camera.local/live",
        Some("user:password"),
        "http://192.168.1.65:2020/onvif/device_service",
    );

    let error = Config::parse(&input)
        .expect("configuration should parse")
        .decrypt_secrets(&SecretManager::from_key(TEST_KEY))
        .expect_err("HTTP is not a valid RTSP URL");

    assert!(!error.to_string().contains("super-secret-rtsp-url"));
}

#[test]
fn rejects_incomplete_onvif_configuration() {
    let input = config_with_values(
        "rtsp://camera.local/live",
        None,
        "http://192.168.1.65:2020/onvif/device_service",
    );

    let error = Config::parse(&input)
        .expect("configuration should parse")
        .decrypt_secrets(&SecretManager::from_key(TEST_KEY))
        .expect_err("ONVIF requires both fields");

    assert_eq!(
        error.to_string(),
        "invalid configuration: onvif_url and onvif_credentials must be set together"
    );
}

#[test]
fn rejects_onvif_url_with_credentials() {
    let input = config_with_values(
        "rtsp://camera.local/live",
        Some("user:password"),
        "http://user:password@192.168.1.65:2020/onvif/device_service",
    );

    let error = Config::parse(&input)
        .expect("configuration should parse")
        .decrypt_secrets(&SecretManager::from_key(TEST_KEY))
        .expect_err("ONVIF URL must not contain credentials");

    assert_eq!(
        error.to_string(),
        "invalid configuration: onvif_url must not contain credentials"
    );
}

#[test]
fn rejects_a_segment_rotation_longer_than_the_rolling_buffer() {
    let input = valid_config().replace(
        "rolling_buffer_seconds = 30",
        "rolling_buffer_seconds = 10\nsegment_rotation_seconds = 11",
    );

    let error = Config::parse(&input)
        .expect("configuration should parse")
        .decrypt_secrets(&SecretManager::from_key(TEST_KEY))
        .expect_err("rotation cannot exceed the rolling buffer");

    assert_eq!(
        error.to_string(),
        "invalid configuration: segment_rotation_seconds cannot exceed rolling_buffer_seconds"
    );
}

#[test]
fn parses_encrypted_r2_values_when_enabled() {
    let input = valid_config().replace(
        "rolling_buffer_seconds = 30",
        &format!(
            "rolling_buffer_seconds = 30\n\
r2_enabled = true\n\
r2_endpoint = \"{}\"\n\
r2_access_key_id = \"{}\"\n\
r2_secret_access_key = \"{}\"\n\
r2_bucket = \"{}\"\n\
r2_prefix = \"{}\"\n\
r2_region = \"{}\"\n",
            secret("https://r2.example.com"),
            secret("access-key"),
            secret("secret-key"),
            secret("bucket"),
            secret("clips"),
            secret("auto"),
        ),
    );

    let config = parse_config(&input);

    assert!(config.app.r2_enabled);
    assert_eq!(
        config.app.r2_endpoint.as_ref().unwrap(),
        "https://r2.example.com"
    );
    assert_eq!(config.app.r2_region.as_ref().unwrap(), "auto");
}

#[test]
fn rejects_enabled_r2_without_required_values() {
    let input = valid_config().replace(
        "rolling_buffer_seconds = 30",
        "rolling_buffer_seconds = 30\nr2_enabled = true",
    );

    let error = Config::parse(&input)
        .expect("configuration should parse")
        .decrypt_secrets(&SecretManager::from_key(TEST_KEY))
        .expect_err("enabled R2 should require values");

    let message = error.to_string();
    assert!(message.contains("r2_endpoint is required when r2_enabled is true"));
    assert!(message.contains("r2_access_key_id is required when r2_enabled is true"));
    assert!(message.contains("r2_secret_access_key is required when r2_enabled is true"));
    assert!(message.contains("r2_bucket is required when r2_enabled is true"));
}
