use axum::http::{HeaderValue, StatusCode};
use camwatch_server::router::router_with_session_expiry;
use time::Duration;
use tower::ServiceExt;

mod support;
use support::{
    cookie_request, csrf_token, form_request, login_with_default_credentials, response_body,
    test_context,
};

#[tokio::test]
async fn ptz_command_is_rejected_when_camera_has_no_ptz() {
    let context = test_context().await;
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
            "/cameras/front-door/ptz/up",
            &cookie,
            &format!("csrf_token={token}"),
        ))
        .await
        .expect("PTZ request should complete");

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response_body(response).await;
    assert!(body.contains("PTZ is not available for this camera."));
    assert!(!body.contains("camera.local"));
}

#[tokio::test]
async fn invalid_ptz_direction_is_not_a_camera_command() {
    let context = test_context().await;
    let app = router_with_session_expiry(context.state, Duration::hours(1));
    let cookie = login_with_default_credentials(&app).await;

    let response = app
        .oneshot(form_request(
            "/cameras/front-door/ptz/spin",
            &cookie,
            "csrf_token=unused",
        ))
        .await
        .expect("invalid PTZ request should complete");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn htmx_ptz_error_replaces_only_the_feedback_fragment() {
    let context = test_context().await;
    let app = router_with_session_expiry(context.state, Duration::hours(1));
    let cookie = login_with_default_credentials(&app).await;
    let details_page = app
        .clone()
        .oneshot(cookie_request("/cameras/front-door", &cookie))
        .await
        .expect("camera details request should complete");
    let token = csrf_token(&response_body(details_page).await);
    let mut request = form_request(
        "/cameras/front-door/ptz/up",
        &cookie,
        &format!("csrf_token={token}"),
    );
    request
        .headers_mut()
        .insert("HX-Request", HeaderValue::from_static("true"));

    let response = app
        .oneshot(request)
        .await
        .expect("htmx PTZ request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body(response).await;
    assert!(body.contains("id=\"ptz-feedback\""));
    assert!(!body.contains("<form"));
    assert!(!body.contains("ptz-grid"));
}
