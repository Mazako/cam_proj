use std::{future::Future, pin::Pin};

use crate::clips::Clip;
use thiserror::Error;

pub type BucketFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait BucketUploader: Send + Sync {
    fn upload(
        &self,
        request: UploadRequest,
    ) -> BucketFuture<'_, Result<RemoteObject, BucketUploaderError>>;
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
