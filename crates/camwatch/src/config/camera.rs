use serde::Deserialize;
use thiserror::Error;
use url::Url;

use crate::stream::RtspCodec;

use super::{CameraId, SecretError, SecretManager};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraConfig {
    pub id: CameraId,
    pub name: String,
    pub rtsp_url: String,
    #[serde(default)]
    pub rtsp_codec: RtspCodec,
    pub onvif_url: Option<Url>,
    pub onvif_credentials: Option<String>,
    pub motion_min_area: u32,
    pub yolo_confidence: f32,
    #[serde(default = "default_clip_after_motion")]
    pub clip_after_motion: bool,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum CameraValidationError {
    #[error("camera ID may contain only lowercase letters, digits, and hyphens")]
    InvalidId,
    #[error("camera name cannot be empty")]
    EmptyName,
    #[error("rtsp_url must be a valid RTSP URL")]
    InvalidRtspUrl,
    #[error("rtsp codec must be h264 or h265")]
    InvalidRtspCodec,
    #[error("motion_min_area must be greater than zero")]
    InvalidMotionMinArea,
    #[error("yolo_confidence must be between 0 and 1")]
    InvalidYoloConfidence,
    #[error("onvif_url and onvif_credentials must be set together")]
    IncompleteOnvif,
    #[error("onvif_credentials must not be empty")]
    InvalidOnvifCredentials,
    #[error("onvif_url must be a valid HTTP or HTTPS URL")]
    InvalidOnvifUrl,
    #[error("onvif_url must use the http or https scheme")]
    InvalidOnvifScheme,
    #[error("onvif_url must not contain credentials")]
    OnvifUrlContainsCredentials,
}

pub struct CameraConfigParts {
    pub id: CameraId,
    pub name: String,
    pub rtsp_url: String,
    pub rtsp_codec: RtspCodec,
    pub onvif_url: Option<Url>,
    pub onvif_credentials: Option<String>,
    pub motion_min_area: u32,
    pub yolo_confidence: f32,
    pub clip_after_motion: bool,
}

pub struct CameraConfigInput {
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

impl CameraConfigInput {
    pub fn validate(&self) -> Result<CameraConfig, Vec<CameraValidationError>> {
        let mut errors = Vec::new();
        if let Err(name_errors) = CameraConfig::validate_name(&self.name) {
            errors.extend(name_errors);
        }
        let id = match CameraId::parse(self.id.trim().to_owned()) {
            Ok(id) => Some(id),
            Err(_) => {
                errors.push(CameraValidationError::InvalidId);
                None
            }
        };
        let rtsp_url = match Url::parse(self.rtsp_url.trim()) {
            Ok(url) if matches!(url.scheme(), "rtsp" | "rtsps") => Some(url.to_string()),
            _ => {
                errors.push(CameraValidationError::InvalidRtspUrl);
                None
            }
        };
        let rtsp_codec = match RtspCodec::parse_storage(self.rtsp_codec.trim()) {
            Some(codec) => Some(codec),
            None => {
                errors.push(CameraValidationError::InvalidRtspCodec);
                None
            }
        };
        let onvif_url = match self.onvif_url.trim() {
            "" => None,
            value => match Url::parse(value) {
                Ok(url) => Some(url),
                Err(_) => {
                    errors.push(CameraValidationError::InvalidOnvifUrl);
                    None
                }
            },
        };
        let onvif_credentials = match self.onvif_credentials.trim() {
            "" => None,
            value => Some(value.to_owned()),
        };
        let motion_min_area = match self.motion_min_area.trim().parse::<u32>() {
            Ok(value) if value > 0 => Some(value),
            _ => {
                errors.push(CameraValidationError::InvalidMotionMinArea);
                None
            }
        };
        let yolo_confidence = match self.yolo_confidence.trim().parse::<f32>() {
            Ok(value) if value.is_finite() && (0.0..=1.0).contains(&value) => Some(value),
            _ => {
                errors.push(CameraValidationError::InvalidYoloConfidence);
                None
            }
        };
        let (
            Some(id),
            Some(rtsp_url),
            Some(rtsp_codec),
            Some(motion_min_area),
            Some(yolo_confidence),
        ) = (id, rtsp_url, rtsp_codec, motion_min_area, yolo_confidence)
        else {
            return Err(errors);
        };
        if !errors.is_empty() {
            return Err(errors);
        }
        CameraConfig::from_parts(CameraConfigParts {
            id,
            name: self.name.trim().to_owned(),
            rtsp_url,
            rtsp_codec,
            onvif_url,
            onvif_credentials,
            motion_min_area,
            yolo_confidence,
            clip_after_motion: self.clip_after_motion,
        })
    }
}

impl CameraConfig {
    pub fn from_parts(parts: CameraConfigParts) -> Result<Self, Vec<CameraValidationError>> {
        let camera = Self {
            id: parts.id,
            name: parts.name,
            rtsp_url: parts.rtsp_url,
            rtsp_codec: parts.rtsp_codec,
            onvif_url: parts.onvif_url,
            onvif_credentials: parts.onvif_credentials,
            motion_min_area: parts.motion_min_area,
            yolo_confidence: parts.yolo_confidence,
            clip_after_motion: parts.clip_after_motion,
        };
        camera.validate()?;
        Ok(camera)
    }

    pub fn validate(&self) -> Result<(), Vec<CameraValidationError>> {
        let mut errors = Vec::new();
        if !is_camera_id(self.id.as_str()) {
            errors.push(CameraValidationError::InvalidId);
        }
        if let Err(name_errors) = Self::validate_name(&self.name) {
            errors.extend(name_errors);
        }
        let rtsp_url = match Url::parse(&self.rtsp_url) {
            Ok(url) if matches!(url.scheme(), "rtsp" | "rtsps") => Some(url),
            _ => {
                errors.push(CameraValidationError::InvalidRtspUrl);
                None
            }
        };
        if rtsp_url.is_none() {
            return Err(errors);
        }
        if self.motion_min_area == 0 {
            errors.push(CameraValidationError::InvalidMotionMinArea);
        }
        if !self.yolo_confidence.is_finite() || !(0.0..=1.0).contains(&self.yolo_confidence) {
            errors.push(CameraValidationError::InvalidYoloConfidence);
        }
        if self.onvif_url.is_some() != self.onvif_credentials.is_some() {
            errors.push(CameraValidationError::IncompleteOnvif);
        }
        if let Some(credentials) = &self.onvif_credentials
            && credentials.trim().is_empty()
        {
            errors.push(CameraValidationError::InvalidOnvifCredentials);
        }
        if let Some(onvif_url) = &self.onvif_url {
            if !matches!(onvif_url.scheme(), "http" | "https") {
                errors.push(CameraValidationError::InvalidOnvifScheme);
            }
            if !onvif_url.username().is_empty() || onvif_url.password().is_some() {
                errors.push(CameraValidationError::OnvifUrlContainsCredentials);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn validate_name(name: &str) -> Result<(), Vec<CameraValidationError>> {
        if name.trim().is_empty() {
            return Err(vec![CameraValidationError::EmptyName]);
        }
        Ok(())
    }

    pub fn from_storage(
        camera: crate::storage::Camera,
        secrets: &SecretManager,
    ) -> Result<Self, crate::config::ConfigError> {
        let rtsp_url = secrets.decrypt(&camera.rtsp_url)?;
        let onvif_credentials = camera
            .onvif_credentials
            .map(|value| secrets.decrypt(&value))
            .transpose()?;
        let mut errors = Vec::new();
        let id = match CameraId::parse(camera.id) {
            Ok(id) => Some(id),
            Err(_) => {
                errors.push(CameraValidationError::InvalidId);
                None
            }
        };
        let rtsp_codec = match RtspCodec::parse_storage(&camera.rtsp_codec) {
            Some(codec) => Some(codec),
            None => {
                errors.push(CameraValidationError::InvalidRtspCodec);
                None
            }
        };
        let onvif_url = match camera.onvif_url {
            Some(value) => match Url::parse(&value) {
                Ok(url) => Some(url),
                Err(_) => {
                    errors.push(CameraValidationError::InvalidOnvifUrl);
                    None
                }
            },
            None => None,
        };
        let motion_min_area = match camera.motion_min_area.try_into() {
            Ok(value) => Some(value),
            Err(_) => {
                errors.push(CameraValidationError::InvalidMotionMinArea);
                None
            }
        };
        let yolo_confidence = camera.yolo_confidence as f32;
        let (Some(id), Some(rtsp_codec), Some(motion_min_area)) = (id, rtsp_codec, motion_min_area)
        else {
            return Err(errors.into());
        };
        if !errors.is_empty() {
            return Err(errors.into());
        }
        Self::from_parts(CameraConfigParts {
            id,
            name: camera.name,
            rtsp_url,
            rtsp_codec,
            onvif_url,
            onvif_credentials,
            motion_min_area,
            yolo_confidence,
            clip_after_motion: camera.clip_after_motion,
        })
        .map_err(Into::into)
    }

    pub fn decrypt_secrets(&mut self, secrets: &SecretManager) -> Result<(), SecretError> {
        self.rtsp_url = secrets.decrypt(&self.rtsp_url)?;
        self.onvif_credentials = self
            .onvif_credentials
            .take()
            .map(|value| secrets.decrypt(&value))
            .transpose()?;
        Ok(())
    }

    pub fn to_storage(
        &self,
        secrets: &SecretManager,
    ) -> Result<crate::storage::NewCamera, SecretError> {
        Ok(crate::storage::NewCamera {
            id: self.id.as_str().to_owned(),
            name: self.name.clone(),
            rtsp_url: secrets.encrypt(&self.rtsp_url)?,
            rtsp_codec: self.rtsp_codec.as_str().to_owned(),
            onvif_url: self.onvif_url.as_ref().map(ToString::to_string),
            onvif_credentials: self
                .onvif_credentials
                .as_ref()
                .map(|value| secrets.encrypt(value))
                .transpose()?,
            motion_min_area: i64::from(self.motion_min_area),
            yolo_confidence: f64::from(self.yolo_confidence),
            clip_after_motion: self.clip_after_motion,
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
