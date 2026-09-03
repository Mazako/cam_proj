use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use axum::{Router, http::StatusCode, routing::get};
use tower_http::trace::TraceLayer;

use crate::{app_state::AppState, error::NonLoopbackBindAddress};

pub fn validate_bind_address(address: SocketAddr) -> Result<(), NonLoopbackBindAddress> {
    let is_allowed = match address.ip() {
        IpAddr::V4(ip) => ip == Ipv4Addr::LOCALHOST,
        IpAddr::V6(ip) => ip == Ipv6Addr::LOCALHOST,
    };

    is_allowed
        .then_some(())
        .ok_or(NonLoopbackBindAddress(address))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok\n")
}
