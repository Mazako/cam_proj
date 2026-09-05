use std::{path::Path, sync::Arc, time::SystemTime};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use camwatch::{
    config::{Config, SecretManager},
    stream::CameraStreamStatus,
};
use camwatch_server::{
    app_state::{AppState, bootstrap_with_secret_manager},
    router::router_with_session_expiry,
};
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

#[tokio::test]
async fn camera_can_be_created_and_saved_when_runtime_is_unavailable() {
    let context = test_context().await;
    let state = context.state.clone();
    let app = router_with_session_expiry(context.state, Duration::hours(1));
    let cookie = login_with_default_credentials(&app).await;
    let new_page = app
        .clone()
        .oneshot(cookie_request("/cameras/new", &cookie))
        .await
        .expect("new camera page request should complete");
    let token = csrf_token(&response_body(new_page).await);

    let response = app
        .oneshot(form_request(
            "/cameras/new",
            &cookie,
            &format!(
                "csrf_token={token}&id=back-yard&name=Back%20yard&rtsp_url=rtsp://camera.local/back-yard&motion_min_area=1000&yolo_confidence=0.5&clip_after_motion=on"
            ),
        ))
        .await
        .expect("camera creation request should complete");

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/cameras/back-yard");
    let camera = state
        .database
        .get_camera("back-yard")
        .await
        .expect("created camera should load")
        .expect("created camera should exist");
    assert_eq!(camera.name, "Back yard");
    assert!(camera.rtsp_url.starts_with("enc:v1:aes256gcm:"));
    assert_eq!(
        state.secret_manager.decrypt(&camera.rtsp_url).unwrap(),
        "rtsp://camera.local/back-yard"
    );
}

#[tokio::test]
async fn camera_can_be_edited_without_reloading_other_runtime_state() {
    let context = test_context().await;
    let state = context.state.clone();
    let app = router_with_session_expiry(context.state, Duration::hours(1));
    let cookie = login_with_default_credentials(&app).await;
    let edit_page = app
        .clone()
        .oneshot(cookie_request("/cameras/front-door/edit", &cookie))
        .await
        .expect("camera edit page request should complete");
    let token = csrf_token(&response_body(edit_page).await);

    let response = app
        .oneshot(form_request(
            "/cameras/front-door/edit",
            &cookie,
            &format!(
                "csrf_token={token}&id=front-door&name=Updated%20front%20door&rtsp_url=&motion_min_area=2000&yolo_confidence=0.7"
            ),
        ))
        .await
        .expect("camera update request should complete");

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let camera = state
        .database
        .get_camera("front-door")
        .await
        .expect("updated camera should load")
        .expect("updated camera should exist");
    assert_eq!(camera.name, "Updated front door");
    assert_eq!(camera.motion_min_area, 2000);
    assert!((camera.yolo_confidence - 0.7).abs() < f64::from(f32::EPSILON));
    assert!(!camera.clip_after_motion);
    assert_eq!(
        state.secret_manager.decrypt(&camera.rtsp_url).unwrap(),
        "rtsp://camera.local/front-door"
    );
}

#[tokio::test]
async fn invalid_camera_input_renders_errors_without_saving() {
    let context = test_context().await;
    let state = context.state.clone();
    let app = router_with_session_expiry(context.state, Duration::hours(1));
    let cookie = login_with_default_credentials(&app).await;
    let new_page = app
        .clone()
        .oneshot(cookie_request("/cameras/new", &cookie))
        .await
        .expect("new camera page request should complete");
    let token = csrf_token(&response_body(new_page).await);

    let response = app
        .oneshot(form_request(
            "/cameras/new",
            &cookie,
            &format!(
                "csrf_token={token}&id=Bad%20ID&name=&rtsp_url=http://secret&motion_min_area=0&yolo_confidence=2"
            ),
        ))
        .await
        .expect("invalid camera request should complete");

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = response_body(response).await;
    assert!(body.contains("camera ID may contain only lowercase letters"));
    assert!(body.contains("camera name cannot be empty"));
    assert!(body.contains("rtsp_url must be a valid RTSP URL"));
    assert_eq!(state.database.camera_count().await.unwrap(), 1);
}

#[tokio::test]
async fn camera_delete_soft_deletes_record_and_stops_runtime() {
    let context = test_context().await;
    let state = context.state.clone();
    let app = router_with_session_expiry(context.state, Duration::hours(1));
    let cookie = login_with_default_credentials(&app).await;
    let details_page = app
        .clone()
        .oneshot(cookie_request("/cameras/front-door", &cookie))
        .await
        .expect("camera details request should complete");
    let token = csrf_token(&response_body(details_page).await);

    let response = app
        .oneshot(form_request(
            "/cameras/front-door/delete",
            &cookie,
            &format!("csrf_token={token}"),
        ))
        .await
        .expect("camera delete request should complete");

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/cameras");
    let camera = state
        .database
        .get_camera("front-door")
        .await
        .expect("deleted camera should load")
        .expect("deleted camera record should be retained");
    assert!(!camera.enabled);
    assert!(camera.deleted_at.is_some());
    assert!(state.camera_summaries().await.unwrap().is_empty());
    assert!(state.camera_details("front-door").await.unwrap().is_none());
}

struct TestContext {
    _directory: tempfile::TempDir,
    state: AppState,
}

async fn test_context() -> TestContext {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let database_path = directory.path().join("camwatch.sqlite3");
    let state = bootstrap_with_secret_manager(
        config(&database_path),
        Arc::new(SecretManager::from_key([9; 32])),
    )
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
rtsp_url = "{}"
motion_min_area = 1000
yolo_confidence = 0.5
clip_after_motion = true
"#,
        database_path.display(),
        SecretManager::from_key([9; 32])
            .encrypt("rtsp://camera.local/front-door")
            .expect("test secret should encrypt")
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

fn form_request(uri: &str, cookie: &str, form: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .expect("form request should be valid")
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
