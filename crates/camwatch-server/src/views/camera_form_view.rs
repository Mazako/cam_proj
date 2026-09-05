use askama::Template;

use crate::camera_dto::{CameraFormError, CameraInput};

#[derive(Debug, Template)]
#[template(path = "pages/camera_form.html")]
pub struct CameraFormView {
    pub csrf_token: String,
    pub show_logout: bool,
    pub title: &'static str,
    pub submit_label: &'static str,
    pub action: String,
    pub camera: CameraFormFields,
    pub errors: Vec<CameraFormError>,
    pub has_errors: bool,
    pub is_edit: bool,
}

#[derive(Debug)]
pub struct CameraFormFields {
    pub id: String,
    pub name: String,
    pub rtsp_url: String,
    pub rtsp_codec: String,
    pub onvif_url: String,
    pub onvif_credentials: String,
    pub motion_min_area: String,
    pub yolo_confidence: String,
    pub clip_after_motion: bool,
}

impl CameraFormView {
    pub fn new(
        csrf_token: String,
        title: &'static str,
        submit_label: &'static str,
        action: String,
        input: CameraInput,
        errors: Vec<CameraFormError>,
        is_edit: bool,
    ) -> Self {
        let has_errors = !errors.is_empty();
        Self {
            csrf_token,
            show_logout: true,
            title,
            submit_label,
            action,
            camera: CameraFormFields::from(input),
            errors,
            has_errors,
            is_edit,
        }
    }
}

impl From<CameraInput> for CameraFormFields {
    fn from(input: CameraInput) -> Self {
        Self {
            id: input.id,
            name: input.name,
            rtsp_url: input.rtsp_url,
            rtsp_codec: input.rtsp_codec,
            onvif_url: input.onvif_url,
            onvif_credentials: input.onvif_credentials,
            motion_min_area: input.motion_min_area,
            yolo_confidence: input.yolo_confidence,
            clip_after_motion: input.clip_after_motion.is_some(),
        }
    }
}
