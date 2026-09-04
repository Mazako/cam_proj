mod camera_details_view;
mod camera_form_view;
mod camera_list_view;
mod error_403_view;
mod error_404_view;
mod error_500_view;
mod home_view;
mod login_view;

use askama_web::WebTemplateExt;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub use camera_details_view::CameraDetailsView;
pub use camera_form_view::CameraFormView;
pub use camera_list_view::CameraListView;
pub use error_403_view::Error403View;
pub use error_404_view::Error404View;
pub use error_500_view::Error500View;
pub use home_view::HomeView;
pub use login_view::LoginView;

pub fn home_page_response(csrf_token: String) -> Response {
    HomeView::new(csrf_token)
        .into_web_template()
        .into_response()
}

pub fn camera_list_page_response(
    csrf_token: String,
    cameras: Vec<crate::camera_dto::CameraSummaryDto>,
) -> Response {
    CameraListView::new(csrf_token, cameras)
        .into_web_template()
        .into_response()
}

pub fn camera_details_page_response(
    csrf_token: String,
    camera: crate::camera_dto::CameraDetailsDto,
) -> Response {
    CameraDetailsView::new(csrf_token, camera)
        .into_web_template()
        .into_response()
}

pub fn camera_form_response(status: StatusCode, view: CameraFormView) -> Response {
    (status, view.into_web_template()).into_response()
}

pub fn not_found_response() -> Response {
    (
        StatusCode::NOT_FOUND,
        Error404View::new().into_web_template(),
    )
        .into_response()
}

pub fn forbidden_response() -> Response {
    (
        StatusCode::FORBIDDEN,
        Error403View::new().into_web_template(),
    )
        .into_response()
}

pub fn internal_error_response() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Error500View::new().into_web_template(),
    )
        .into_response()
}

pub fn login_page_response(csrf_token: String, show_error: bool) -> Response {
    LoginView::new(csrf_token, show_error)
        .into_web_template()
        .into_response()
}
