use std::fmt;

use serde::{Deserialize, Deserializer};

#[derive(Clone, PartialEq, Eq)]
pub struct EnvironmentVariableName(String);

impl EnvironmentVariableName {
    pub fn parse(value: String) -> Result<Self, &'static str> {
        if is_environment_variable_name(&value) {
            Ok(Self(value))
        } else {
            Err("invalid environment variable name")
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

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

pub(super) fn is_environment_variable_name(value: &str) -> bool {
    let mut characters = value.bytes();
    matches!(characters.next(), Some(character) if character.is_ascii_uppercase() || character == b'_')
        && characters.all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == b'_'
        })
}
