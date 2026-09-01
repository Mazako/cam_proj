use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use camwatch::{
    clips::{Clip, ClipUploadJob, create_clip_uploader_worker},
    ports::{BucketUploader, BucketUploaderError, PortFuture, RemoteObject, UploadRequest},
};

struct FakeUploader {
    attempts: Arc<AtomicUsize>,
    failures_before_success: usize,
}

impl BucketUploader for FakeUploader {
    fn upload(
        &self,
        _request: UploadRequest,
    ) -> PortFuture<'_, Result<RemoteObject, BucketUploaderError>> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        let failures_before_success = self.failures_before_success;
        Box::pin(async move {
            if attempt <= failures_before_success {
                Err(BucketUploaderError::Failed)
            } else {
                Ok(RemoteObject {
                    key: "events/event-1.mp4".to_owned(),
                    etag: None,
                    verbose: true,
                })
            }
        })
    }
}

#[tokio::test]
async fn retries_upload_until_it_succeeds() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let uploader = Arc::new(FakeUploader {
        attempts: Arc::clone(&attempts),
        failures_before_success: 2,
    });
    let sender = create_clip_uploader_worker(uploader);

    sender
        .send(upload_job())
        .expect("clip upload job should be queued");
    wait_for_attempts(&attempts, 3).await;

    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn stops_after_three_upload_attempts() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let uploader = Arc::new(FakeUploader {
        attempts: Arc::clone(&attempts),
        failures_before_success: 10,
    });
    let sender = create_clip_uploader_worker(uploader);

    sender
        .send(upload_job())
        .expect("clip upload job should be queued");
    wait_for_attempts(&attempts, 3).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

fn upload_job() -> ClipUploadJob {
    ClipUploadJob {
        camera_id: "front-door".to_owned(),
        request: UploadRequest {
            event_id: "event-1".to_owned(),
            clip: Clip::new("clip.mp4".into(), Duration::from_secs(1)),
        },
    }
}

async fn wait_for_attempts(attempts: &AtomicUsize, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while attempts.load(Ordering::SeqCst) < expected {
        assert!(
            Instant::now() < deadline,
            "upload worker did not retry in time"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
