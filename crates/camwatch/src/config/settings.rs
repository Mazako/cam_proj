use std::{fs, path::Path};

use serde::Deserialize;

use super::{
    AppConfig, CameraConfig, ConfigError, camera::is_camera_id,
    environment::is_environment_variable_name,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub app: AppConfig,
    #[serde(default)]
    pub cameras: Vec<CameraConfig>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&contents)
    }

    pub fn parse(contents: &str) -> Result<Self, ConfigError> {
        let config = toml::from_str::<Self>(contents).map_err(|_| ConfigError::InvalidToml)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.app.pre_event_seconds > self.app.rolling_buffer_seconds {
            return Err(ConfigError::Validation(
                "pre_event_seconds cannot exceed rolling_buffer_seconds",
            ));
        }
        if self.app.post_event_seconds == 0 || self.app.rolling_buffer_seconds == 0 {
            return Err(ConfigError::Validation(
                "post_event_seconds and rolling_buffer_seconds must be greater than zero",
            ));
        }
        if self.app.segment_rotation_seconds == 0 {
            return Err(ConfigError::Validation(
                "segment_rotation_seconds must be greater than zero",
            ));
        }
        if self.app.segment_rotation_seconds > self.app.rolling_buffer_seconds {
            return Err(ConfigError::Validation(
                "segment_rotation_seconds cannot exceed rolling_buffer_seconds",
            ));
        }
        if self.app.r2_enabled {
            if self.app.r2_endpoint_env.is_none() {
                return Err(ConfigError::Validation(
                    "r2_endpoint_env is required when r2_enabled is true",
                ));
            }
            if self.app.r2_access_key_id_env.is_none() {
                return Err(ConfigError::Validation(
                    "r2_access_key_id_env is required when r2_enabled is true",
                ));
            }
            if self.app.r2_secret_access_key_env.is_none() {
                return Err(ConfigError::Validation(
                    "r2_secret_access_key_env is required when r2_enabled is true",
                ));
            }
            if self.app.r2_bucket_env.is_none() {
                return Err(ConfigError::Validation(
                    "r2_bucket_env is required when r2_enabled is true",
                ));
            }
        }

        let mut camera_ids = std::collections::HashSet::new();
        for camera in &self.cameras {
            if !is_camera_id(camera.id.as_str()) {
                return Err(ConfigError::Validation(
                    "camera ID may contain only lowercase letters, digits, and hyphens",
                ));
            }
            if !camera_ids.insert(camera.id.as_str()) {
                return Err(ConfigError::Validation("camera IDs must be unique"));
            }
            if camera.name.trim().is_empty() {
                return Err(ConfigError::Validation("camera name cannot be empty"));
            }
            if !is_environment_variable_name(camera.rtsp_url_env.as_str()) {
                return Err(ConfigError::Validation(
                    "rtsp_url_env must be an environment variable name",
                ));
            }
            if camera.motion_min_area == 0 {
                return Err(ConfigError::Validation(
                    "motion_min_area must be greater than zero",
                ));
            }
            if !(0.0..=1.0).contains(&camera.yolo_confidence) {
                return Err(ConfigError::Validation(
                    "yolo_confidence must be between 0 and 1",
                ));
            }
            if camera.onvif_url.is_some() != camera.onvif_credentials_env.is_some() {
                return Err(ConfigError::Validation(
                    "onvif_url and onvif_credentials_env must be set together",
                ));
            }
            if let Some(environment_variable) = &camera.onvif_credentials_env
                && !is_environment_variable_name(environment_variable.as_str())
            {
                return Err(ConfigError::Validation(
                    "onvif_credentials_env must be an environment variable name",
                ));
            }
            if let Some(onvif_url) = &camera.onvif_url
                && !matches!(onvif_url.scheme(), "http" | "https")
            {
                return Err(ConfigError::Validation(
                    "onvif_url must use the http or https scheme",
                ));
            }
        }
        Ok(())
    }
}
