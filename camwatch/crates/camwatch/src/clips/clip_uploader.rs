use std::{sync::Arc, time::Duration};

use backon::{ExponentialBuilder, Retryable};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::ports::{BucketUploader, BucketUploaderError, UploadRequest};

pub struct ClipUploadJob {
    pub camera_id: String,
    pub request: UploadRequest,
}

pub fn create_clip_uploader_worker(
    uploader: Arc<dyn BucketUploader>,
) -> UnboundedSender<ClipUploadJob> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(run_worker(rx, uploader));
    tx
}

async fn run_worker(mut rx: UnboundedReceiver<ClipUploadJob>, uploader: Arc<dyn BucketUploader>) {
    while let Some(job) = rx.recv().await {
        let camera_id = job.camera_id.clone();
        let request = job.request;
        let result = upload_with_retries(Arc::clone(&uploader), request).await;

        match result {
            Ok(remote_object) if remote_object.verbose => {
                tracing::info!(
                    camera_id,
                    object_key = remote_object.key.as_str(),
                    "clip uploaded"
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::error!(camera_id, %error, "clip upload failed after three attempts");
            }
        }
    }
}

async fn upload_with_retries(
    uploader: Arc<dyn BucketUploader>,
    request: UploadRequest,
) -> Result<crate::ports::RemoteObject, BucketUploaderError> {
    let backoff = ExponentialBuilder::default()
        .with_jitter()
        .with_min_delay(Duration::from_secs(1))
        .with_max_delay(Duration::from_secs(30))
        .with_max_times(3);

    (|| uploader.upload(request.clone())).retry(backoff).await
}
