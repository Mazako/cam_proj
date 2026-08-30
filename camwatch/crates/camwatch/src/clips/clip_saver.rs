use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use super::{clip_manager::ClipJob, clip_store::create_clip_from_segments};

pub fn create_clip_worker() -> UnboundedSender<ClipJob> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(run_worker(rx));
    tx
}

async fn run_worker(mut rx: UnboundedReceiver<ClipJob>) {
    while let Some(job) = rx.recv().await {
        let ClipJob {
            camera_id,
            started_at: _,
            ended_at: _,
            path,
            segments,
            _lease,
        } = job;

        if let Err(error) = create_clip_from_segments(segments, path).await {
            tracing::error!(
                camera_id = camera_id.as_str(),
                %error,
                "clip could not be created"
            );
        }
    }
}
