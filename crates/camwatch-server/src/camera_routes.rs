use axum::{
    extract::{Form, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    app_state::AppState,
    auth_routes,
    camera_dto::{CameraFormError, CameraInput},
    error::{PtzCommandError, RuntimeReloadError},
    views::{self, CameraFormView, PtzFeedbackView},
};

#[derive(Debug, Deserialize)]
pub(crate) struct CsrfForm {
    csrf_token: String,
}

pub(crate) async fn new_page(session: Session) -> Response {
    let csrf_token = match auth_routes::csrf_token(&session).await {
        Ok(token) => token,
        Err(_) => return views::internal_error_response(),
    };
    views::camera_form_response(
        StatusCode::OK,
        CameraFormView::new(
            csrf_token,
            "Add camera",
            "Add camera",
            "/cameras/new".to_owned(),
            CameraInput::default(),
            Vec::new(),
            false,
        ),
    )
}

pub(crate) async fn create(
    State(state): State<AppState>,
    session: Session,
    Form(input): Form<CameraInput>,
) -> Response {
    if !auth_routes::valid_csrf(&session, &input.csrf_token).await {
        return views::forbidden_response();
    }

    let csrf_token = match auth_routes::csrf_token(&session).await {
        Ok(token) => token,
        Err(_) => return views::internal_error_response(),
    };
    let camera = match input.validate() {
        Ok(camera) => camera,
        Err(errors) => {
            return camera_form_error_response(
                csrf_token,
                "Add camera",
                "Add camera",
                "/cameras/new".to_owned(),
                input,
                errors.into_iter().map(CameraFormError::from).collect(),
                false,
            );
        }
    };
    let camera_id = camera.id.as_str().to_owned();
    let new_camera = match camera.to_storage(&state.secret_manager) {
        Ok(camera) => camera,
        Err(error) => {
            tracing::error!(camera_id, %error, "failed to encrypt camera configuration");
            return views::internal_error_response();
        }
    };

    match state.database.get_camera(&camera_id).await {
        Ok(Some(camera)) if camera.deleted_at.is_none() && camera.enabled => {
            return camera_form_error_response(
                csrf_token,
                "Add camera",
                "Add camera",
                "/cameras/new".to_owned(),
                input,
                vec![CameraFormError::AlreadyExists],
                false,
            );
        }
        Ok(_) => {}
        Err(_) => {
            tracing::error!(camera_id, "failed to check existing camera");
            return views::internal_error_response();
        }
    }

    if let Err(error) = state.database.upsert_camera(&new_camera).await {
        tracing::error!(camera_id, %error, "failed to save camera");
        return views::internal_error_response();
    }
    reload_camera_runtime(&state, &camera_id).await;
    Redirect::to(&format!("/cameras/{camera_id}")).into_response()
}

pub(crate) async fn edit_page(
    State(state): State<AppState>,
    Path(camera_id): Path<String>,
    session: Session,
) -> Response {
    let csrf_token = match auth_routes::csrf_token(&session).await {
        Ok(token) => token,
        Err(_) => return views::internal_error_response(),
    };
    let details = match state.camera_details(&camera_id).await {
        Ok(Some(details)) => details,
        Ok(None) => return views::not_found_response(),
        Err(_) => {
            tracing::error!(camera_id, "failed to load camera for editing");
            return views::internal_error_response();
        }
    };
    views::camera_form_response(
        StatusCode::OK,
        CameraFormView::new(
            csrf_token,
            "Edit camera",
            "Save changes",
            format!("/cameras/{camera_id}/edit"),
            CameraInput::from(details),
            Vec::new(),
            true,
        ),
    )
}

pub(crate) async fn update(
    State(state): State<AppState>,
    Path(camera_id): Path<String>,
    session: Session,
    Form(mut input): Form<CameraInput>,
) -> Response {
    if !auth_routes::valid_csrf(&session, &input.csrf_token).await {
        return views::forbidden_response();
    }

    let csrf_token = match auth_routes::csrf_token(&session).await {
        Ok(token) => token,
        Err(_) => return views::internal_error_response(),
    };
    let stored_camera = match state.database.get_camera(&camera_id).await {
        Ok(Some(camera)) if camera.enabled && camera.deleted_at.is_none() => camera,
        Ok(_) => return views::not_found_response(),
        Err(_) => {
            tracing::error!(camera_id, "failed to check camera before editing");
            return views::internal_error_response();
        }
    };
    let stored_config =
        match camwatch::config::CameraConfig::from_storage(stored_camera, &state.secret_manager) {
            Ok(camera) => camera,
            Err(error) => {
                tracing::error!(camera_id, %error, "failed to decrypt camera before editing");
                return views::internal_error_response();
            }
        };
    if input.rtsp_url.trim().is_empty() {
        input.rtsp_url = stored_config.rtsp_url.clone();
    }
    if !input.onvif_url.trim().is_empty() && input.onvif_credentials.trim().is_empty() {
        input.onvif_credentials = stored_config.onvif_credentials.clone().unwrap_or_default();
    }
    let action = format!("/cameras/{camera_id}/edit");
    let mut errors = Vec::new();
    if input.id != camera_id {
        errors.push(CameraFormError::IdCannotChange);
    }
    let new_camera = match input.validate() {
        Ok(camera) => Some(camera),
        Err(validation_errors) => {
            errors.extend(validation_errors.into_iter().map(CameraFormError::from));
            None
        }
    };
    if !errors.is_empty() {
        return camera_form_error_response(
            csrf_token,
            "Edit camera",
            "Save changes",
            action,
            input,
            errors,
            true,
        );
    }
    let new_camera = new_camera.expect("valid camera input should be present");
    let new_camera = match new_camera.to_storage(&state.secret_manager) {
        Ok(camera) => camera,
        Err(error) => {
            tracing::error!(camera_id, %error, "failed to encrypt camera configuration");
            return views::internal_error_response();
        }
    };

    if let Err(error) = state.database.upsert_camera(&new_camera).await {
        tracing::error!(camera_id, %error, "failed to update camera");
        return views::internal_error_response();
    }
    reload_camera_runtime(&state, &camera_id).await;
    Redirect::to(&format!("/cameras/{camera_id}")).into_response()
}

pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(camera_id): Path<String>,
    session: Session,
    Form(form): Form<CsrfForm>,
) -> Response {
    if !auth_routes::valid_csrf(&session, &form.csrf_token).await {
        return views::forbidden_response();
    }
    match state.database.soft_delete_camera(&camera_id).await {
        Ok(true) => {}
        Ok(false) => return views::not_found_response(),
        Err(_) => {
            tracing::error!(camera_id, "failed to delete camera");
            return views::internal_error_response();
        }
    }
    state.stop_runtime(&camera_id).await;
    Redirect::to("/cameras").into_response()
}

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

pub(crate) async fn ptz(
    State(state): State<AppState>,
    Path((camera_id, direction)): Path<(String, String)>,
    headers: HeaderMap,
    session: Session,
    Form(form): Form<CsrfForm>,
) -> Response {
    let Some(direction) = parse_ptz_direction(&direction) else {
        return views::not_found_response();
    };
    if !auth_routes::valid_csrf(&session, &form.csrf_token).await {
        return views::forbidden_response();
    }
    let csrf_token = match auth_routes::csrf_token(&session).await {
        Ok(token) => token,
        Err(_) => return views::internal_error_response(),
    };
    let is_htmx = headers
        .get("HX-Request")
        .and_then(|value| value.to_str().ok())
        == Some("true");

    match state.move_ptz(&camera_id, direction).await {
        Ok(()) => {
            if is_htmx {
                views::ptz_feedback_response(
                    StatusCode::OK,
                    PtzFeedbackView::new("Movement completed.", "muted"),
                )
            } else {
                Redirect::to(&format!("/cameras/{camera_id}")).into_response()
            }
        }
        Err(error) => {
            let (status, message) = match error {
                PtzCommandError::Unavailable => (
                    StatusCode::CONFLICT,
                    "PTZ is not available for this camera.",
                ),
                PtzCommandError::Failed => (
                    StatusCode::BAD_GATEWAY,
                    "Camera movement failed. Try again.",
                ),
            };
            if is_htmx {
                views::ptz_feedback_response(
                    StatusCode::OK,
                    PtzFeedbackView::new(message, "form-error"),
                )
            } else {
                ptz_error_page(&state, &camera_id, csrf_token, status, message).await
            }
        }
    }
}

async fn reload_camera_runtime(state: &AppState, camera_id: &str) {
    let camera = match state.database.get_camera(camera_id).await {
        Ok(Some(camera)) => camera,
        Ok(None) => {
            tracing::error!(camera_id, "saved camera could not be loaded");
            return;
        }
        Err(error) => {
            tracing::error!(camera_id, %error, "saved camera could not be loaded");
            return;
        }
    };
    if let Err(error) = state.replace_camera_runtime(camera).await {
        match error {
            RuntimeReloadError::StreamUnavailable => {
                tracing::warn!(camera_id, "camera runtime could not start")
            }
            RuntimeReloadError::InvalidConfiguration(_) => {
                tracing::error!(camera_id, "saved camera configuration is invalid")
            }
        }
    }
}

fn parse_ptz_direction(direction: &str) -> Option<camwatch::onvif::PtzDirection> {
    const SPEED: f32 = 0.5;
    match direction {
        "up" => Some(camwatch::onvif::PtzDirection::Up(SPEED)),
        "down" => Some(camwatch::onvif::PtzDirection::Down(SPEED)),
        "left" => Some(camwatch::onvif::PtzDirection::Left(SPEED)),
        "right" => Some(camwatch::onvif::PtzDirection::Right(SPEED)),
        _ => None,
    }
}

async fn ptz_error_page(
    state: &AppState,
    camera_id: &str,
    csrf_token: String,
    status: StatusCode,
    message: &'static str,
) -> Response {
    match state.camera_details(camera_id).await {
        Ok(Some(camera)) => views::camera_details_page_response_with_ptz_message(
            status,
            csrf_token,
            camera,
            message,
            "form-error",
        ),
        Ok(None) => views::not_found_response(),
        Err(_) => views::internal_error_response(),
    }
}

fn camera_form_error_response(
    csrf_token: String,
    title: &'static str,
    submit_label: &'static str,
    action: String,
    input: CameraInput,
    errors: Vec<CameraFormError>,
    is_edit: bool,
) -> Response {
    views::camera_form_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        CameraFormView::new(
            csrf_token,
            title,
            submit_label,
            action,
            input,
            errors,
            is_edit,
        ),
    )
}
