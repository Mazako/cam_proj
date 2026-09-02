use aws_config::BehaviorVersion;
use aws_sdk_s3::{Client, config::Credentials, primitives::ByteStream};

use crate::config::AppConfig;

use super::{
    BucketFuture, BucketUploader, BucketUploaderError, R2Config, R2Error, RemoteObject,
    UploadRequest,
};

pub struct R2Client {
    client: Client,
    bucket: String,
    prefix: String,
}

impl R2Client {
    pub fn from_app_config(app_config: &AppConfig) -> Result<Self, R2Error> {
        let config = R2Config::from_app_config(app_config)?;
        Self::new(config)
    }

    pub fn new(config: R2Config) -> Result<Self, R2Error> {
        let sdk_config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .endpoint_url(config.endpoint.as_str())
            .region(aws_sdk_s3::config::Region::new(config.region))
            .credentials_provider(Credentials::new(
                config.access_key_id,
                config.secret_access_key,
                None,
                None,
                "CAMWATCH_R2",
            ))
            .build();

        Ok(Self {
            client: Client::from_conf(sdk_config),
            bucket: config.bucket,
            prefix: config.prefix,
        })
    }

    async fn upload_clip(&self, request: UploadRequest) -> Result<RemoteObject, R2Error> {
        let key = self.object_key(&request.event_id);
        let body = ByteStream::from_path(&request.clip.path)
            .await
            .map_err(|error| R2Error::ReadClip(error.to_string()))?;

        let response = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .content_type("video/mp4")
            .body(body)
            .send()
            .await
            .map_err(|error| R2Error::Upload(Box::new(error)))?;

        Ok(RemoteObject {
            key,
            etag: response.e_tag().map(ToOwned::to_owned),
            verbose: true,
        })
    }

    fn object_key(&self, event_id: &str) -> String {
        format!("{}{event_id}.mp4", self.prefix)
    }
}

impl BucketUploader for R2Client {
    fn upload(
        &self,
        request: UploadRequest,
    ) -> BucketFuture<'_, Result<RemoteObject, BucketUploaderError>> {
        Box::pin(async move {
            self.upload_clip(request)
                .await
                .map_err(|_| BucketUploaderError::Failed)
        })
    }
}
