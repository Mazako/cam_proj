use std::fmt;

use url::Url;

use crate::config::AppConfig;

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
        let endpoint = app_config
            .r2_endpoint
            .as_ref()
            .ok_or(R2Error::MissingConfiguration("r2_endpoint"))?;
        let endpoint = Url::parse(endpoint).map_err(|_| R2Error::InvalidEndpoint)?;
        if !matches!(endpoint.scheme(), "http" | "https") || endpoint.host_str().is_none() {
            return Err(R2Error::InvalidEndpoint);
        }

        let access_key_id = app_config
            .r2_access_key_id
            .clone()
            .ok_or(R2Error::MissingConfiguration("r2_access_key_id"))?;
        let secret_access_key = app_config
            .r2_secret_access_key
            .clone()
            .ok_or(R2Error::MissingConfiguration("r2_secret_access_key"))?;
        let bucket = app_config
            .r2_bucket
            .clone()
            .ok_or(R2Error::MissingConfiguration("r2_bucket"))?;
        let prefix = app_config.r2_prefix.clone().unwrap_or_default();
        let region = app_config
            .r2_region
            .clone()
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
