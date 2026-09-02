use std::{env, fmt};

use url::Url;

use crate::config::{AppConfig, EnvironmentVariableName};

use super::R2Error;

#[derive(Clone)]
pub struct R2Config {
    pub(crate) endpoint: Url,
    pub(crate) access_key_id: String,
    pub(crate) secret_access_key: String,
    pub(crate) bucket: String,
    pub(crate) prefix: String,
    pub(crate) region: String,
}

impl fmt::Debug for R2Config {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("R2Config")
            .field("endpoint", &self.endpoint)
            .field("access_key_id", &"[REDACTED]")
            .field("secret_access_key", &"[REDACTED]")
            .field("bucket", &self.bucket)
            .field("prefix", &self.prefix)
            .field("region", &self.region)
            .finish()
    }
}

impl R2Config {
    pub fn from_app_config(app_config: &AppConfig) -> Result<Self, R2Error> {
        let endpoint_environment_variable =
            required_reference(app_config.r2_endpoint_env.as_ref(), "r2_endpoint_env")?;
        let endpoint = required_environment_variable(endpoint_environment_variable)?;
        let endpoint = Url::parse(&endpoint).map_err(|_| R2Error::InvalidEndpoint)?;
        if !matches!(endpoint.scheme(), "http" | "https") || endpoint.host_str().is_none() {
            return Err(R2Error::InvalidEndpoint);
        }

        let access_key_id = required_environment_variable(required_reference(
            app_config.r2_access_key_id_env.as_ref(),
            "r2_access_key_id_env",
        )?)?;
        let secret_access_key = required_environment_variable(required_reference(
            app_config.r2_secret_access_key_env.as_ref(),
            "r2_secret_access_key_env",
        )?)?;
        let bucket = required_environment_variable(required_reference(
            app_config.r2_bucket_env.as_ref(),
            "r2_bucket_env",
        )?)?;
        let prefix =
            optional_environment_variable(app_config.r2_prefix_env.as_ref())?.unwrap_or_default();
        let region = optional_environment_variable(app_config.r2_region_env.as_ref())?
            .unwrap_or_else(|| "auto".to_owned());

        if bucket.trim().is_empty() || region.trim().is_empty() {
            return Err(R2Error::InvalidConfiguration);
        }

        Ok(Self {
            endpoint,
            access_key_id,
            secret_access_key,
            bucket,
            prefix,
            region,
        })
    }
}

fn required_reference<'a>(
    reference: Option<&'a EnvironmentVariableName>,
    field: &'static str,
) -> Result<&'a str, R2Error> {
    reference
        .map(EnvironmentVariableName::as_str)
        .ok_or(R2Error::MissingEnvironmentVariableReference(field))
}

fn required_environment_variable(name: &str) -> Result<String, R2Error> {
    let value = env::var(name).map_err(|_| R2Error::MissingEnvironmentVariable(name.to_owned()))?;
    if value.trim().is_empty() {
        return Err(R2Error::MissingEnvironmentVariable(name.to_owned()));
    }
    Ok(value)
}

fn optional_environment_variable(
    reference: Option<&EnvironmentVariableName>,
) -> Result<Option<String>, R2Error> {
    reference
        .map(|reference| required_environment_variable(reference.as_str()))
        .transpose()
}
