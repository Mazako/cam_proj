use serde::Deserialize;
use url::Url;

use super::environment::EnvironmentVariableName;
use crate::stream::RtspCodec;

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

fn default_clip_after_motion() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct CameraId(String);

impl CameraId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub(super) fn is_camera_id(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == b'-'
        })
}
