use camwatch::config::{CameraConfig, CameraConfigInput, CameraValidationError};
use serde::Deserialize;
use thiserror::Error;

use super::CameraDetailsDto;

#[derive(Clone, Debug, Deserialize)]
pub struct CameraInput {
    pub csrf_token: String,
    pub id: String,
    pub name: String,
    pub rtsp_url: String,
    #[serde(default)]
    pub onvif_url: String,
    #[serde(default)]
    pub onvif_credentials: String,
    pub motion_min_area: String,
    pub yolo_confidence: String,
    #[serde(default)]
    pub clip_after_motion: Option<String>,
}

#[derive(Debug, Error)]
pub enum CameraFormError {
    #[error(transparent)]
    Validation(#[from] CameraValidationError),
    #[error("A camera with this ID already exists.")]
    AlreadyExists,
    #[error("Camera ID cannot be changed.")]
    IdCannotChange,
}

impl CameraInput {
    pub fn validate(&self) -> Result<CameraConfig, Vec<CameraValidationError>> {
        let camera = CameraConfigInput {
            id: self.id.clone(),
            name: self.name.clone(),
            rtsp_url: self.rtsp_url.clone(),
            onvif_url: self.onvif_url.clone(),
            onvif_credentials: self.onvif_credentials.clone(),
            motion_min_area: self.motion_min_area.clone(),
            yolo_confidence: self.yolo_confidence.clone(),
            clip_after_motion: self.clip_after_motion.is_some(),
        }
        .validate()?;

        Ok(camera)
    }
}

impl Default for CameraInput {
    fn default() -> Self {
        Self {
            csrf_token: String::new(),
            id: String::new(),
            name: String::new(),
            rtsp_url: String::new(),
            onvif_url: String::new(),
            onvif_credentials: String::new(),
            motion_min_area: "1000".to_owned(),
            yolo_confidence: "0.5".to_owned(),
            clip_after_motion: Some("on".to_owned()),
        }
    }
}

impl From<CameraDetailsDto> for CameraInput {
    fn from(camera: CameraDetailsDto) -> Self {
        Self {
            csrf_token: String::new(),
            id: camera.summary.id,
            name: camera.summary.name,
            rtsp_url: String::new(),
            onvif_url: camera.onvif_url.unwrap_or_default(),
            onvif_credentials: String::new(),
            motion_min_area: camera.motion_min_area.to_string(),
            yolo_confidence: camera.yolo_confidence.to_string(),
            clip_after_motion: camera.clip_after_motion.then(|| "on".to_owned()),
        }
    }
}
