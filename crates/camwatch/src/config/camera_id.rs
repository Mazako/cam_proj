use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct CameraId(String);

impl CameraId {
    pub fn parse(value: String) -> Result<Self, &'static str> {
        if !value.is_empty()
            && value.bytes().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == b'-'
            })
        {
            Ok(Self(value))
        } else {
            Err("invalid camera ID")
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
