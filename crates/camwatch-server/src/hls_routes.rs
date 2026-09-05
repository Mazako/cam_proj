use std::{io::ErrorKind, path::PathBuf};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use camwatch::config::CameraId;

use crate::app_state::AppState;

const PLAYLIST_CONTENT_TYPE: &str = "application/vnd.apple.mpegurl";
const SEGMENT_CONTENT_TYPE: &str = "video/mp2t";

pub(crate) async fn playlist(
    State(state): State<AppState>,
    Path(camera_id): Path<String>,
) -> Response {
    serve_file(&state, &camera_id, "index.m3u8", PLAYLIST_CONTENT_TYPE).await
}

pub(crate) async fn segment(
    State(state): State<AppState>,
    Path((camera_id, segment_name)): Path<(String, String)>,
) -> Response {
    if !is_safe_segment_name(&segment_name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    serve_file(&state, &camera_id, &segment_name, SEGMENT_CONTENT_TYPE).await
}

async fn serve_file(
    state: &AppState,
    camera_id: &str,
    file_name: &str,
    content_type: &str,
) -> Response {
    let Some(directory) = hls_directory(state, camera_id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let path = directory.join(file_name);

    match tokio::fs::read(path).await {
        Ok(contents) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CACHE_CONTROL, "no-store")
            .body(Body::from(contents))
            .expect("HLS response should be valid"),
        Err(error) if error.kind() == ErrorKind::NotFound => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(camera_id, %error, "HLS file could not be read");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn hls_directory(state: &AppState, camera_id: &str) -> Option<PathBuf> {
    CameraId::parse(camera_id.to_owned()).ok()?;
    state
        .camera_runtimes
        .get(camera_id)?
        .is_running()
        .then_some(())?;
    Some(state.runtime_config.hls_directory.join(camera_id))
}

fn is_safe_segment_name(value: &str) -> bool {
    let Some(index) = value
        .strip_prefix("segment-")
        .and_then(|value| value.strip_suffix(".ts"))
    else {
        return false;
    };
    !index.is_empty() && index.len() <= 5 && index.bytes().all(|byte| byte.is_ascii_digit())
}
