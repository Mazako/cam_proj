use std::fs;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use camwatch_server::router::router_with_session_expiry;
use time::Duration;
use tower::ServiceExt;

mod support;
use support::{cookie_request, login_with_default_credentials, response_body, test_context};

#[tokio::test]
async fn serves_authenticated_hls_playlist_and_segments() {
    let context = test_context().await;
    let hls_directory = context
        .state
        .runtime_config
        .hls_directory
        .join("front-door");
    fs::create_dir_all(&hls_directory).expect("HLS directory should exist");
    fs::write(
        hls_directory.join("index.m3u8"),
        "#EXTM3U\n#EXTINF:2.0,\nsegment-00000.ts\n",
    )
    .expect("HLS playlist should be written");
    fs::write(hls_directory.join("segment-00000.ts"), b"segment")
        .expect("HLS segment should be written");

    let app = router_with_session_expiry(context.state, Duration::hours(1));
    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/hls/front-door/index.m3u8")
                .body(Body::empty())
                .expect("HLS request should be valid"),
        )
        .await
        .expect("unauthorized HLS request should complete");
    assert_eq!(unauthorized.status(), StatusCode::SEE_OTHER);

    let cookie = login_with_default_credentials(&app).await;
    let playlist = app
        .clone()
        .oneshot(cookie_request("/hls/front-door/index.m3u8", &cookie))
        .await
        .expect("playlist request should complete");
    assert_eq!(playlist.status(), StatusCode::OK);
    assert_eq!(
        playlist.headers()[header::CONTENT_TYPE],
        "application/vnd.apple.mpegurl"
    );
    assert_eq!(
        response_body(playlist).await,
        "#EXTM3U\n#EXTINF:2.0,\nsegment-00000.ts\n"
    );

    let segment = app
        .clone()
        .oneshot(cookie_request("/hls/front-door/segment-00000.ts", &cookie))
        .await
        .expect("segment request should complete");
    assert_eq!(segment.status(), StatusCode::OK);
    assert_eq!(segment.headers()[header::CONTENT_TYPE], "video/mp2t");
    assert_eq!(response_body(segment).await, "segment");

    let invalid_segment = app
        .clone()
        .oneshot(cookie_request("/hls/front-door/not-a-segment.ts", &cookie))
        .await
        .expect("invalid segment request should complete");
    assert_eq!(invalid_segment.status(), StatusCode::NOT_FOUND);

    let missing_camera = app
        .oneshot(cookie_request("/hls/missing/index.m3u8", &cookie))
        .await
        .expect("missing camera request should complete");
    assert_eq!(missing_camera.status(), StatusCode::NOT_FOUND);
}
