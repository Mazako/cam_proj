use std::time::Duration as StdDuration;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use camwatch_server::router::router_with_session_expiry;
use time::Duration;
use tower::ServiceExt;

mod support;
use support::{
    cookie_request, csrf_token, form_request, login_with_default_credentials, response_body,
    session_cookie, test_context,
};

#[tokio::test]
async fn protected_routes_redirect_to_login() {
    let context = test_context().await;
    let state = context.state.clone();
    let app = router_with_session_expiry(state, Duration::hours(1));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/cameras")
                .body(Body::empty())
                .expect("protected request should be valid"),
        )
        .await
        .expect("protected request should complete");

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/login");
}

#[tokio::test]
async fn login_sets_local_session_cookie_and_logout_invalidates_it() {
    let context = test_context().await;
    let state = context.state.clone();
    let app = router_with_session_expiry(state, Duration::hours(1));

    let login_page = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/login")
                .body(Body::empty())
                .expect("login request should be valid"),
        )
        .await
        .expect("login request should complete");
    let initial_cookie = session_cookie(&login_page);
    let cookie_header = login_page
        .headers()
        .get(header::SET_COOKIE)
        .expect("login page should set a session cookie")
        .to_str()
        .expect("session cookie should be valid HTTP")
        .to_owned();
    let login_body = response_body(login_page).await;
    let login_csrf_token = csrf_token(&login_body);

    assert!(!login_body.contains("admin"));
    assert!(cookie_header.contains("HttpOnly"));
    assert!(cookie_header.contains("SameSite=Strict"));
    assert!(cookie_header.contains("Path=/"));
    assert!(cookie_header.contains("Max-Age=3600"));
    assert!(!cookie_header.contains("Secure"));
    assert!(!cookie_header.contains("Domain="));

    let login_response = app
        .clone()
        .oneshot(form_request(
            "/login",
            &initial_cookie,
            &format!("login=admin&password=admin&csrf_token={login_csrf_token}"),
        ))
        .await
        .expect("login submission should complete");
    assert_eq!(login_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(login_response.headers()[header::LOCATION], "/cameras");
    let authenticated_cookie = session_cookie(&login_response);

    let authenticated_page = app
        .clone()
        .oneshot(cookie_request("/cameras", &authenticated_cookie))
        .await
        .expect("authenticated request should complete");
    assert_eq!(authenticated_page.status(), StatusCode::OK);
    let authenticated_body = response_body(authenticated_page).await;
    let csrf_token = csrf_token(&authenticated_body);

    let logout_response = app
        .clone()
        .oneshot(form_request(
            "/logout",
            &authenticated_cookie,
            &format!("csrf_token={csrf_token}"),
        ))
        .await
        .expect("logout submission should complete");
    assert_eq!(logout_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(logout_response.headers()[header::LOCATION], "/login");
    assert!(
        logout_response.headers()[header::SET_COOKIE]
            .to_str()
            .expect("logout cookie should be valid HTTP")
            .contains("Max-Age=0")
    );

    let after_logout = app
        .oneshot(cookie_request("/cameras", &authenticated_cookie))
        .await
        .expect("request after logout should complete");
    assert_eq!(after_logout.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn invalid_login_is_generic_and_csrf_is_required() {
    let context = test_context().await;
    let state = context.state.clone();
    let app = router_with_session_expiry(state, Duration::hours(1));
    let login_page = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/login")
                .body(Body::empty())
                .expect("login request should be valid"),
        )
        .await
        .expect("login request should complete");
    let cookie = session_cookie(&login_page);
    let csrf_token = csrf_token(&response_body(login_page).await);

    let invalid_response = app
        .clone()
        .oneshot(form_request(
            "/login",
            &cookie,
            &format!("login=admin&password=wrong&csrf_token={csrf_token}"),
        ))
        .await
        .expect("invalid login should complete");
    assert_eq!(invalid_response.status(), StatusCode::OK);
    let invalid_body = response_body(invalid_response).await;
    assert!(invalid_body.contains("Invalid login or password."));
    assert!(!invalid_body.contains("wrong"));

    let csrf_response = app
        .oneshot(form_request(
            "/login",
            &cookie,
            "login=admin&password=admin&csrf_token=invalid",
        ))
        .await
        .expect("invalid csrf request should complete");
    assert_eq!(csrf_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn session_expires_after_inactivity() {
    let context = test_context().await;
    let state = context.state.clone();
    let app = router_with_session_expiry(state, Duration::seconds(1));
    let cookie = login_with_default_credentials(&app).await;

    tokio::time::sleep(StdDuration::from_secs(2)).await;

    let response = app
        .oneshot(cookie_request("/cameras", &cookie))
        .await
        .expect("expired session request should complete");
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn session_does_not_survive_router_restart() {
    let context = test_context().await;
    let state = context.state.clone();
    let app = router_with_session_expiry(state.clone(), Duration::hours(1));
    let cookie = login_with_default_credentials(&app).await;
    let restarted_app = router_with_session_expiry(state, Duration::hours(1));

    let response = restarted_app
        .oneshot(cookie_request("/cameras", &cookie))
        .await
        .expect("request after restart should complete");
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
}
