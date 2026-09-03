use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use camwatch_server::router::{health_router, validate_bind_address};
use tower::ServiceExt;

#[tokio::test]
async fn health_endpoint_does_not_start_camera_integrations() {
    let response = health_router()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("health request should be valid"),
        )
        .await
        .expect("health request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "text/plain; charset=utf-8"
    );
}

#[test]
fn only_ipv4_and_ipv6_localhost_bind_addresses_are_allowed() {
    assert!(validate_bind_address(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080,)).is_ok());
    assert!(validate_bind_address(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 8080,)).is_ok());

    for address in [
        SocketAddr::from(([0, 0, 0, 0], 8080)),
        SocketAddr::from(([127, 0, 0, 2], 8080)),
        SocketAddr::from(([192, 168, 1, 10], 8080)),
        SocketAddr::from(([0u16; 8], 8080)),
    ] {
        assert!(
            validate_bind_address(address).is_err(),
            "{address} must be rejected"
        );
    }
}
