use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::bucket::UploadRequest;

use super::{ClipJob, ClipUploadJob, clip_store::create_clip_from_segments};

pub fn create_clip_worker(
    upload_sender: UnboundedSender<ClipUploadJob>,
) -> UnboundedSender<ClipJob> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(run_worker(rx, upload_sender));
    tx
}

async fn run_worker(
    mut rx: UnboundedReceiver<ClipJob>,
    upload_sender: UnboundedSender<ClipUploadJob>,
) {
    while let Some(job) = rx.recv().await {
        let ClipJob {
            event_id,
            camera_id,
            started_at: _,
            ended_at: _,
            path,
            segments,
            _lease,
        } = job;

        match create_clip_from_segments(segments, path).await {
            Ok(clip) => {
                let upload_job = ClipUploadJob {
                    camera_id,
                    request: UploadRequest { event_id, clip },
                };
                if upload_sender.send(upload_job).is_err() {
                    tracing::warn!("clip uploader is unavailable");
                }
            }
            Err(error) => {
                tracing::error!(%error, "clip could not be created");
            }
        }
    }
}
