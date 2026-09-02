use std::{future::Future, pin::Pin};

use super::{BucketUploaderError, RemoteObject, UploadRequest};

pub type BucketFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait BucketUploader: Send + Sync {
    fn upload(
        &self,
        request: UploadRequest,
    ) -> BucketFuture<'_, Result<RemoteObject, BucketUploaderError>>;
}
