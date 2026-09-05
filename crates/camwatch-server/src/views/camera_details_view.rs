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
    pub hls_playlist_url: String,
    pub stream_status: &'static str,
    pub runtime_status: &'static str,
    pub ptz_status: &'static str,
    pub ptz_available: bool,
    pub ptz_panel_visible: bool,
    pub ptz_message: String,
    pub ptz_message_class: &'static str,
}

impl CameraDetailsView {
    pub fn new(csrf_token: String, camera: CameraDetailsDto) -> Self {
        Self {
            csrf_token,
            show_logout: true,
            camera: CameraDetailsItemView::from(camera),
        }
    }

    pub fn with_ptz_message(
        mut self,
        message: impl Into<String>,
        message_class: &'static str,
    ) -> Self {
        self.camera.ptz_message = message.into();
        self.camera.ptz_message_class = message_class;
        self.camera.ptz_panel_visible = true;
        self
    }
}

impl From<CameraDetailsDto> for CameraDetailsItemView {
    fn from(camera: CameraDetailsDto) -> Self {
        let id = camera.summary.id;
        Self {
            hls_playlist_url: format!("/hls/{id}/index.m3u8"),
            id,
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
            ptz_available: camera.summary.ptz_available,
            ptz_panel_visible: camera.summary.ptz_available,
            ptz_message: String::new(),
            ptz_message_class: "muted",
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
