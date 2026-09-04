use axum::{
    extract::{Path, State},
    response::Response,
};
use tower_sessions::Session;

use crate::{app_state::AppState, auth_routes, views};

pub(crate) async fn list(State(state): State<AppState>, session: Session) -> Response {
    let csrf_token = match auth_routes::csrf_token(&session).await {
        Ok(token) => token,
        Err(_) => return views::internal_error_response(),
    };

    match state.camera_summaries().await {
        Ok(cameras) => views::camera_list_page_response(csrf_token, cameras),
        Err(_) => {
            tracing::error!("failed to load camera list");
            views::internal_error_response()
        }
    }
}

pub(crate) async fn details(
    State(state): State<AppState>,
    Path(camera_id): Path<String>,
    session: Session,
) -> Response {
    let csrf_token = match auth_routes::csrf_token(&session).await {
        Ok(token) => token,
        Err(_) => return views::internal_error_response(),
    };

    match state.camera_details(&camera_id).await {
        Ok(Some(camera)) => views::camera_details_page_response(csrf_token, camera),
        Ok(None) => views::not_found_response(),
        Err(_) => {
            tracing::error!(camera_id, "failed to load camera details");
            views::internal_error_response()
        }
    }
}
