use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use axum::{Router, http::StatusCode, middleware, routing::get};
use time::Duration;
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{
    app_state::AppState,
    auth_routes::{self, login, login_page, logout, protected_home, require_auth},
    camera_routes,
    error::NonLoopbackBindAddress,
    views,
};

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
    router_with_session_expiry(state, Duration::hours(1))
}

pub fn router_with_session_expiry(state: AppState, expiry: Duration) -> Router {
    public_routes()
        .merge(protected_routes())
        .fallback(not_found)
        .layer(auth_routes::session_layer(expiry))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub fn health_router() -> Router {
    Router::new()
        .route("/health", get(health))
        .layer(TraceLayer::new_for_http())
}

fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/login", get(login_page).post(login))
        .nest_service(
            "/assets",
            ServeDir::new(concat!(env!("CARGO_MANIFEST_DIR"), "/static")),
        )
}

fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(protected_home))
        .route("/cameras", get(camera_routes::list))
        .route(
            "/cameras/new",
            get(camera_routes::new_page).post(camera_routes::create),
        )
        .route("/cameras/{camera_id}", get(camera_routes::details))
        .route(
            "/cameras/{camera_id}/edit",
            get(camera_routes::edit_page).post(camera_routes::update),
        )
        .route(
            "/cameras/{camera_id}/delete",
            axum::routing::post(camera_routes::delete),
        )
        .route("/logout", axum::routing::post(logout))
        .layer(middleware::from_fn(require_auth))
}

async fn health() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok\n")
}

async fn not_found() -> axum::response::Response {
    views::not_found_response()
}
