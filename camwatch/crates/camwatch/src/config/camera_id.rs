use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct CameraId(String);

impl CameraId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
