use super::{BucketFuture, BucketUploader, BucketUploaderError, RemoteObject, UploadRequest};

pub struct NoOpBucketUploader;

impl BucketUploader for NoOpBucketUploader {
    fn upload(
        &self,
        request: UploadRequest,
    ) -> BucketFuture<'_, Result<RemoteObject, BucketUploaderError>> {
        Box::pin(async move {
            Ok(RemoteObject {
                key: format!("noop/{}.mp4", request.event_id),
                etag: None,
                verbose: false,
            })
        })
    }
}
