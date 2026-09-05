use std::{path::Path, sync::Arc};

use axum::{
    body::{Body, to_bytes},
    http::{Request, header},
};
use camwatch::config::{Config, SecretManager};
use camwatch_server::app_state::{AppState, bootstrap_with_secret_manager};
use tower::ServiceExt;

pub struct TestContext {
    pub state: AppState,
    _directory: tempfile::TempDir,
}

pub async fn test_context() -> TestContext {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let database_path = directory.path().join("camwatch.sqlite3");
    let state = bootstrap_with_secret_manager(
        config(&database_path),
        Arc::new(SecretManager::from_key([9; 32])),
    )
    .await
    .expect("test server should bootstrap");
    TestContext {
        state,
        _directory: directory,
    }
}

pub fn config(database_path: &Path) -> Config {
    let hls_directory = database_path
        .parent()
        .expect("database path should have a parent")
        .join("hls");
    let contents = format!(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "{}"
pre_event_seconds = 10
post_event_seconds = 20
rolling_buffer_seconds = 30
hls_directory = "{}"

[[cameras]]
id = "front-door"
name = "Front door"
rtsp_url = "{}"
motion_min_area = 1000
yolo_confidence = 0.5
clip_after_motion = true
"#,
        database_path.display(),
        hls_directory.display(),
        SecretManager::from_key([9; 32])
            .encrypt("rtsp://camera.local/front-door")
            .expect("test secret should encrypt"),
    );
    Config::parse(&contents).expect("test configuration should parse")
}

pub async fn login_with_default_credentials(app: &axum::Router) -> String {
    let login_page = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/login")
                .body(Body::empty())
                .expect("login request should be valid"),
        )
        .await
        .expect("login page request should complete");
    let cookie = session_cookie(&login_page);
    let csrf_token = csrf_token(&response_body(login_page).await);
    let response = app
        .clone()
        .oneshot(form_request(
            "/login",
            &cookie,
            &format!("login=admin&password=admin&csrf_token={csrf_token}"),
        ))
        .await
        .expect("login submission should complete");
    assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER);
    session_cookie(&response)
}

pub fn cookie_request(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .expect("cookie request should be valid")
}

pub fn form_request(uri: &str, cookie: &str, form: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .expect("form request should be valid")
}

pub fn session_cookie(response: &axum::response::Response) -> String {
    session_cookie_from_header(
        response
            .headers()
            .get(header::SET_COOKIE)
            .expect("response should set a session cookie"),
    )
}

pub fn session_cookie_from_header(value: &axum::http::HeaderValue) -> String {
    value
        .to_str()
        .expect("session cookie should be valid HTTP")
        .split(';')
        .next()
        .expect("session cookie should contain a value")
        .to_owned()
}

pub async fn response_body(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    String::from_utf8(bytes.to_vec()).expect("response body should be UTF-8")
}

pub fn csrf_token(body: &str) -> String {
    let marker = r#"name="csrf_token" value=""#;
    let start = body
        .find(marker)
        .expect("rendered form should contain csrf token")
        + marker.len();
    let end = body[start..]
        .find('"')
        .expect("csrf token should be closed")
        + start;
    body[start..end].to_owned()
}
