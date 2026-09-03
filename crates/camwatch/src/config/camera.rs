use serde::Deserialize;
use url::Url;

use crate::stream::RtspCodec;

use super::{CameraId, ConfigError, EnvironmentVariableName};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraConfig {
    pub id: CameraId,
    pub name: String,
    pub rtsp_url_env: EnvironmentVariableName,
    #[serde(default)]
    pub rtsp_codec: RtspCodec,
    pub onvif_url: Option<Url>,
    pub onvif_credentials_env: Option<EnvironmentVariableName>,
    pub motion_min_area: u32,
    pub yolo_confidence: f32,
    #[serde(default = "default_clip_after_motion")]
    pub clip_after_motion: bool,
}

impl CameraConfig {
    pub fn from_storage(camera: crate::storage::Camera) -> Result<Self, ConfigError> {
        let id = CameraId::parse(camera.id).map_err(ConfigError::Validation)?;
        let rtsp_url_env =
            EnvironmentVariableName::parse(camera.rtsp_url_env).map_err(ConfigError::Validation)?;
        let rtsp_codec = RtspCodec::parse_storage(&camera.rtsp_codec)
            .ok_or(ConfigError::Validation("invalid stored RTSP codec"))?;
        let onvif_url = camera
            .onvif_url
            .map(|value| {
                Url::parse(&value).map_err(|_| ConfigError::Validation("invalid stored ONVIF URL"))
            })
            .transpose()?;
        let onvif_credentials_env = camera
            .onvif_credentials_env
            .map(|value| EnvironmentVariableName::parse(value).map_err(ConfigError::Validation))
            .transpose()?;
        if onvif_url.is_some() != onvif_credentials_env.is_some() {
            return Err(ConfigError::Validation(
                "stored ONVIF configuration is incomplete",
            ));
        }
        if let Some(url) = &onvif_url
            && !matches!(url.scheme(), "http" | "https")
        {
            return Err(ConfigError::Validation("invalid stored ONVIF URL"));
        }
        let motion_min_area = camera
            .motion_min_area
            .try_into()
            .map_err(|_| ConfigError::Validation("invalid stored motion_min_area"))?;
        let yolo_confidence = camera.yolo_confidence as f32;
        if !(0.0..=1.0).contains(&yolo_confidence) {
            return Err(ConfigError::Validation("invalid stored yolo_confidence"));
        }

        Ok(Self {
            id,
            name: camera.name,
            rtsp_url_env,
            rtsp_codec,
            onvif_url,
            onvif_credentials_env,
            motion_min_area,
            yolo_confidence,
            clip_after_motion: camera.clip_after_motion,
        })
    }
}

fn default_clip_after_motion() -> bool {
    true
}

pub(super) fn is_camera_id(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == b'-'
        })
}
