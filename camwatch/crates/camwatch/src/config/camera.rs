use serde::Deserialize;
use url::Url;

use crate::stream::RtspCodec;

use super::{CameraId, EnvironmentVariableName};

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

pub(super) fn is_camera_id(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == b'-'
        })
}
