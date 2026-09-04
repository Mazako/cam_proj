use std::{fs, path::Path};

use serde::Deserialize;

use super::{AppConfig, CameraConfig, ConfigError};

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
        self.app.validate()?;

        let mut camera_ids = std::collections::HashSet::new();
        for camera in &self.cameras {
            if !camera_ids.insert(camera.id.as_str()) {
                return Err(ConfigError::DuplicateCameraIds);
            }
            camera.validate()?;
        }
        Ok(())
    }
}
