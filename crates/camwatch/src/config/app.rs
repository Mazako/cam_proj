use std::{net::SocketAddr, path::PathBuf};

use serde::Deserialize;
use thiserror::Error;

use super::{SecretError, SecretManager};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    pub bind_address: SocketAddr,
    pub database_path: PathBuf,
    pub pre_event_seconds: u32,
    pub post_event_seconds: u32,
    pub rolling_buffer_seconds: u32,
    #[serde(default)]
    pub r2_enabled: bool,
    #[serde(default)]
    pub r2_endpoint: Option<String>,
    #[serde(default)]
    pub r2_access_key_id: Option<String>,
    #[serde(default)]
    pub r2_secret_access_key: Option<String>,
    #[serde(default)]
    pub r2_bucket: Option<String>,
    #[serde(default)]
    pub r2_prefix: Option<String>,
    #[serde(default)]
    pub r2_region: Option<String>,
    #[serde(default = "default_segment_directory")]
    pub segment_directory: PathBuf,
    #[serde(default = "default_clips_directory")]
    pub clips_directory: PathBuf,
    #[serde(default = "default_hls_directory")]
    pub hls_directory: PathBuf,
    #[serde(default = "default_segment_rotation_seconds")]
    pub segment_rotation_seconds: u32,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AppValidationError {
    #[error("pre_event_seconds cannot exceed rolling_buffer_seconds")]
    PreEventExceedsRollingBuffer,
    #[error("post_event_seconds and rolling_buffer_seconds must be greater than zero")]
    InvalidBufferDuration,
    #[error("segment_rotation_seconds must be greater than zero")]
    InvalidSegmentRotation,
    #[error("segment_rotation_seconds cannot exceed rolling_buffer_seconds")]
    SegmentRotationExceedsRollingBuffer,
    #[error("r2_endpoint is required when r2_enabled is true")]
    MissingR2Endpoint,
    #[error("r2_access_key_id is required when r2_enabled is true")]
    MissingR2AccessKeyId,
    #[error("r2_secret_access_key is required when r2_enabled is true")]
    MissingR2SecretAccessKey,
    #[error("r2_bucket is required when r2_enabled is true")]
    MissingR2Bucket,
}

impl AppConfig {
    pub fn decrypt_secrets(&mut self, secrets: &SecretManager) -> Result<(), SecretError> {
        if !self.r2_enabled {
            return Ok(());
        }

        for value in [
            &mut self.r2_endpoint,
            &mut self.r2_access_key_id,
            &mut self.r2_secret_access_key,
            &mut self.r2_bucket,
            &mut self.r2_prefix,
            &mut self.r2_region,
        ]
        .into_iter()
        .flatten()
        {
            *value = secrets.decrypt(value)?;
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), Vec<AppValidationError>> {
        let mut errors = Vec::new();
        if self.pre_event_seconds > self.rolling_buffer_seconds {
            errors.push(AppValidationError::PreEventExceedsRollingBuffer);
        }
        if self.post_event_seconds == 0 || self.rolling_buffer_seconds == 0 {
            errors.push(AppValidationError::InvalidBufferDuration);
        }
        if self.segment_rotation_seconds == 0 {
            errors.push(AppValidationError::InvalidSegmentRotation);
        }
        if self.segment_rotation_seconds > self.rolling_buffer_seconds {
            errors.push(AppValidationError::SegmentRotationExceedsRollingBuffer);
        }
        if self.r2_enabled {
            if self.r2_endpoint.is_none() {
                errors.push(AppValidationError::MissingR2Endpoint);
            }
            if self.r2_access_key_id.is_none() {
                errors.push(AppValidationError::MissingR2AccessKeyId);
            }
            if self.r2_secret_access_key.is_none() {
                errors.push(AppValidationError::MissingR2SecretAccessKey);
            }
            if self.r2_bucket.is_none() {
                errors.push(AppValidationError::MissingR2Bucket);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn default_segment_directory() -> PathBuf {
    PathBuf::from("data/segments")
}

fn default_clips_directory() -> PathBuf {
    PathBuf::from("data/clips")
}

fn default_hls_directory() -> PathBuf {
    PathBuf::from("data/hls")
}

fn default_segment_rotation_seconds() -> u32 {
    2
}
