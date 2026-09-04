use askama::Template;
use camwatch::stream::CameraStreamStatus;

use crate::camera_dto::CameraDetailsDto;

#[derive(Debug, Template)]
#[template(path = "pages/camera_details.html")]
pub struct CameraDetailsView {
    pub csrf_token: String,
    pub show_logout: bool,
    pub camera: CameraDetailsItemView,
}

#[derive(Debug)]
pub struct CameraDetailsItemView {
    pub id: String,
    pub name: String,
    pub stream_status: &'static str,
    pub runtime_status: &'static str,
    pub ptz_status: &'static str,
}

impl CameraDetailsView {
    pub fn new(csrf_token: String, camera: CameraDetailsDto) -> Self {
        Self {
            csrf_token,
            show_logout: true,
            camera: CameraDetailsItemView::from(camera),
        }
    }
}

impl From<CameraDetailsDto> for CameraDetailsItemView {
    fn from(camera: CameraDetailsDto) -> Self {
        Self {
            id: camera.summary.id,
            name: camera.summary.name,
            stream_status: stream_status_label(camera.summary.stream_status),
            runtime_status: if camera.summary.runtime_running {
                "Running"
            } else {
                "Stopped"
            },
            ptz_status: if camera.summary.ptz_available {
                "Available"
            } else {
                "Unavailable"
            },
        }
    }
}

fn stream_status_label(status: Option<CameraStreamStatus>) -> &'static str {
    match status {
        Some(CameraStreamStatus::Online { .. }) => "Online",
        Some(CameraStreamStatus::Offline { .. }) => "Offline",
        None => "Unknown",
    }
}
