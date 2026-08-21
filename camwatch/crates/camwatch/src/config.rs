use std::{fmt, fs, net::SocketAddr, path::Path};

use serde::{Deserialize, Deserializer};
use thiserror::Error;
use url::Url;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub app: AppConfig,
    pub cameras: Vec<CameraConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    pub bind_address: SocketAddr,
    pub pre_event_seconds: u32,
    pub post_event_seconds: u32,
    pub rolling_buffer_seconds: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraConfig {
    pub id: CameraId,
    pub name: String,
    pub rtsp_url_env: EnvironmentVariableName,
    pub onvif_url: Option<Url>,
    pub onvif_credentials_env: Option<EnvironmentVariableName>,
    pub motion_min_area: u32,
    pub yolo_confidence: f32,
}

#[derive(Clone, PartialEq, Eq)]
pub struct EnvironmentVariableName(String);

impl fmt::Debug for EnvironmentVariableName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EnvironmentVariableName([REDACTED])")
    }
}

impl<'de> Deserialize<'de> for EnvironmentVariableName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if is_environment_variable_name(&value) {
            Ok(Self(value))
        } else {
            Err(serde::de::Error::custom(
                "invalid environment variable name",
            ))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct CameraId(String);

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
        if self.cameras.is_empty() {
            return Err(ConfigError::Validation(
                "at least one camera must be configured",
            ));
        }
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

        let mut camera_ids = std::collections::HashSet::new();
        for camera in &self.cameras {
            if !is_camera_id(&camera.id.0) {
                return Err(ConfigError::Validation(
                    "camera ID may contain only lowercase letters, digits, and hyphens",
                ));
            }
            if !camera_ids.insert(&camera.id.0) {
                return Err(ConfigError::Validation("camera IDs must be unique"));
            }
            if camera.name.trim().is_empty() {
                return Err(ConfigError::Validation("camera name cannot be empty"));
            }
            if !is_environment_variable_name(&camera.rtsp_url_env.0) {
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
                && !is_environment_variable_name(&environment_variable.0)
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

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read file {path}: {source}")]
    Read {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("invalid configuration")]
    InvalidToml,
    #[error("invalid configuration: {0}")]
    Validation(&'static str),
}

fn is_environment_variable_name(value: &str) -> bool {
    let mut characters = value.bytes();
    matches!(characters.next(), Some(character) if character.is_ascii_uppercase() || character == b'_')
        && characters.all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == b'_'
        })
}

fn is_camera_id(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == b'-'
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CONFIG: &str = r#"
[app]
bind_address = "127.0.0.1:8080"
pre_event_seconds = 10
post_event_seconds = 20
rolling_buffer_seconds = 30

[[cameras]]
id = "front-door"
name = "Front door"
rtsp_url_env = "CAMWATCH_FRONT_DOOR_RTSP_URL"
onvif_url = "http://192.168.1.65:2020/onvif/device_service"
onvif_credentials_env = "CAMWATCH_FRONT_DOOR_ONVIF_CREDENTIALS"
motion_min_area = 1000
yolo_confidence = 0.5
"#;

    #[test]
    fn parses_a_configuration_with_a_camera() {
        let config = Config::parse(VALID_CONFIG).expect("valid configuration should load");

        assert_eq!(config.cameras.len(), 1);
        assert_eq!(config.app.bind_address.to_string(), "127.0.0.1:8080");
    }

    #[test]
    fn rejects_an_invalid_environment_variable_name_without_disclosing_it() {
        let input = VALID_CONFIG.replace(
            "CAMWATCH_FRONT_DOOR_RTSP_URL",
            "rtsp://user:super-secret-rtsp-url@camera.local/live",
        );

        let error = Config::parse(&input)
            .expect_err("a secret value cannot be an environment variable name");

        assert!(!error.to_string().contains("super-secret-rtsp-url"));
    }

    #[test]
    fn rejects_incomplete_onvif_configuration() {
        let input = VALID_CONFIG.replace(
            "onvif_credentials_env = \"CAMWATCH_FRONT_DOOR_ONVIF_CREDENTIALS\"\n",
            "",
        );

        let error = Config::parse(&input).expect_err("ONVIF requires both fields");

        assert_eq!(
            error.to_string(),
            "invalid configuration: onvif_url and onvif_credentials_env must be set together"
        );
    }
}
