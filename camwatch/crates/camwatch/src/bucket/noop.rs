use crate::ports::{BucketUploader, BucketUploaderError, PortFuture, RemoteObject, UploadRequest};

pub struct NoOpBucketUploader;

impl BucketUploader for NoOpBucketUploader {
    fn upload(
        &self,
        request: UploadRequest,
    ) -> PortFuture<'_, Result<RemoteObject, BucketUploaderError>> {
        Box::pin(async move {
            Ok(RemoteObject {
                key: format!("noop/{}.mp4", request.event_id),
                etag: None,
                verbose: false,
            })
        })
    }
}
