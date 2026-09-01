use crate::clips::Clip;
use thiserror::Error;

use super::PortFuture;

pub trait BucketUploader: Send + Sync {
    fn upload(
        &self,
        request: UploadRequest,
    ) -> PortFuture<'_, Result<RemoteObject, BucketUploaderError>>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadRequest {
    pub event_id: String,
    pub clip: Clip,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteObject {
    pub key: String,
    pub etag: Option<String>,
    pub verbose: bool,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BucketUploaderError {
    #[error("bucket is unavailable")]
    Unavailable,
    #[error("bucket upload failed")]
    Failed,
}
