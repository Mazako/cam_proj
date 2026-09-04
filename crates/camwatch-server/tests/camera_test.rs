use std::{path::Path, time::SystemTime};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use camwatch::{config::Config, stream::CameraStreamStatus};
use camwatch_server::{app_state::bootstrap, router::router_with_session_expiry};
use time::Duration;
use tower::ServiceExt;

#[tokio::test]
async fn camera_list_renders_full_ssr_page_with_current_status() {
    let context = test_context().await;
    context.state.status_model.update(
        "front-door",
        CameraStreamStatus::Online {
            since: SystemTime::UNIX_EPOCH,
        },
    );
    let app = router_with_session_expiry(context.state, Duration::hours(1));
    let cookie = login_with_default_credentials(&app).await;

    let response = app
        .oneshot(cookie_request("/cameras", &cookie))
        .await
        .expect("camera list request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body(response).await;
    assert!(body.contains("Front door"));
    assert!(body.contains("Online"));
    assert!(body.contains("href=\"/cameras/front-door\""));
    assert!(!body.contains("hx-"));
}

#[tokio::test]
async fn camera_details_render_full_ssr_page() {
    let context = test_context().await;
    let app = router_with_session_expiry(context.state, Duration::hours(1));
    let cookie = login_with_default_credentials(&app).await;

    let response = app
        .oneshot(cookie_request("/cameras/front-door", &cookie))
        .await
        .expect("camera details request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body(response).await;
    assert!(body.contains("Camera identifier: <code>front-door</code>"));
    assert!(body.contains("Preview is not available yet"));
    assert!(body.contains("<dt>PTZ</dt>"));
    assert!(!body.contains("hx-"));
}

#[tokio::test]
async fn missing_camera_renders_ssr_not_found_page() {
    let context = test_context().await;
    let app = router_with_session_expiry(context.state, Duration::hours(1));
    let cookie = login_with_default_credentials(&app).await;

    let response = app
        .oneshot(cookie_request("/cameras/missing", &cookie))
        .await
        .expect("missing camera request should complete");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(response_body(response).await.contains("Page not found"));
}

struct TestContext {
    _directory: tempfile::TempDir,
    state: camwatch_server::app_state::AppState,
}

async fn test_context() -> TestContext {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let database_path = directory.path().join("camwatch.sqlite3");
    let state = bootstrap(config(&database_path))
        .await
        .expect("test server should bootstrap");
    TestContext {
        _directory: directory,
        state,
    }
}

fn config(database_path: &Path) -> Config {
    let contents = format!(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "{}"
pre_event_seconds = 10
post_event_seconds = 20
rolling_buffer_seconds = 30

[[cameras]]
id = "front-door"
name = "Front door"
rtsp_url_env = "CAMWATCH_CAMERA_TEST_RTSP_URL"
rtsp_codec = "h264"
motion_min_area = 1000
yolo_confidence = 0.5
clip_after_motion = true
"#,
        database_path.display()
    );
    Config::parse(&contents).expect("test configuration should parse")
}

async fn login_with_default_credentials(app: &axum::Router) -> String {
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
    let token = csrf_token(&response_body(login_page).await);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "login=admin&password=admin&csrf_token={token}"
                )))
                .expect("login request should be valid"),
        )
        .await
        .expect("login request should complete");
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    session_cookie(&response)
}

fn cookie_request(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .expect("cookie request should be valid")
}

fn session_cookie(response: &axum::response::Response) -> String {
    response
        .headers()
        .get(header::SET_COOKIE)
        .expect("response should set a session cookie")
        .to_str()
        .expect("session cookie should be valid HTTP")
        .split(';')
        .next()
        .expect("session cookie should contain a value")
        .to_owned()
}

async fn response_body(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    String::from_utf8(bytes.to_vec()).expect("response body should be UTF-8")
}

fn csrf_token(body: &str) -> String {
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
