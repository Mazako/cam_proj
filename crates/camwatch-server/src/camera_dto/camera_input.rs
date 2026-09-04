use camwatch::{
    config::{CameraConfigInput, CameraValidationError},
    storage::NewCamera,
};
use serde::Deserialize;
use thiserror::Error;

use super::CameraDetailsDto;

#[derive(Clone, Debug, Deserialize)]
pub struct CameraInput {
    pub csrf_token: String,
    pub id: String,
    pub name: String,
    pub rtsp_url_env: String,
    pub rtsp_codec: String,
    #[serde(default)]
    pub onvif_url: String,
    #[serde(default)]
    pub onvif_credentials_env: String,
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
    pub fn validate(&self) -> Result<NewCamera, Vec<CameraValidationError>> {
        let camera = CameraConfigInput {
            id: self.id.clone(),
            name: self.name.clone(),
            rtsp_url_env: self.rtsp_url_env.clone(),
            rtsp_codec: self.rtsp_codec.clone(),
            onvif_url: self.onvif_url.clone(),
            onvif_credentials_env: self.onvif_credentials_env.clone(),
            motion_min_area: self.motion_min_area.clone(),
            yolo_confidence: self.yolo_confidence.clone(),
            clip_after_motion: self.clip_after_motion.is_some(),
        }
        .validate()?;

        Ok(camera.into())
    }
}

impl Default for CameraInput {
    fn default() -> Self {
        Self {
            csrf_token: String::new(),
            id: String::new(),
            name: String::new(),
            rtsp_url_env: String::new(),
            rtsp_codec: "h264".to_owned(),
            onvif_url: String::new(),
            onvif_credentials_env: String::new(),
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
            rtsp_url_env: camera.rtsp_url_env,
            rtsp_codec: camera.rtsp_codec,
            onvif_url: camera.onvif_url.unwrap_or_default(),
            onvif_credentials_env: camera.onvif_credentials_env.unwrap_or_default(),
            motion_min_area: camera.motion_min_area.to_string(),
            yolo_confidence: camera.yolo_confidence.to_string(),
            clip_after_motion: camera.clip_after_motion.then(|| "on".to_owned()),
        }
    }
}
