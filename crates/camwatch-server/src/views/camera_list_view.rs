use askama::Template;
use camwatch::stream::CameraStreamStatus;

use crate::camera_dto::CameraSummaryDto;

#[derive(Debug, Template)]
#[template(path = "pages/cameras.html")]
pub struct CameraListView {
    pub csrf_token: String,
    pub show_logout: bool,
    pub cameras: Vec<CameraListItemView>,
    pub has_cameras: bool,
}

#[derive(Debug)]
pub struct CameraListItemView {
    pub id: String,
    pub name: String,
    pub stream_status: &'static str,
    pub runtime_status: &'static str,
    pub ptz_status: &'static str,
}

impl CameraListView {
    pub fn new(csrf_token: String, cameras: Vec<CameraSummaryDto>) -> Self {
        let cameras: Vec<_> = cameras.into_iter().map(CameraListItemView::from).collect();
        let has_cameras = !cameras.is_empty();
        Self {
            csrf_token,
            show_logout: true,
            cameras,
            has_cameras,
        }
    }
}

impl From<CameraSummaryDto> for CameraListItemView {
    fn from(camera: CameraSummaryDto) -> Self {
        Self {
            id: camera.id,
            name: camera.name,
            stream_status: stream_status_label(camera.stream_status),
            runtime_status: if camera.runtime_running {
                "Running"
            } else {
                "Stopped"
            },
            ptz_status: if camera.ptz_available {
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
