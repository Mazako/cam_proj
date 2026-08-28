use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use super::{clip_store::ClipCreationEvent, create_clip};
use crate::storage::Database;

pub fn create_clip_worker(db: Database) -> UnboundedSender<ClipCreationEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(run_worker(rx, db));
    tx
}

async fn run_worker(mut rx: UnboundedReceiver<ClipCreationEvent>, db: Database) {
    while let Some(event) = rx.recv().await {
        if let Err(error) = create_clip(
            &db,
            &event.camera_id,
            event.started_at,
            event.ended_at,
            event.path,
        )
        .await
        {
            tracing::error!(
                camera_id = event.camera_id.as_str(),
                %error,
                "clip could not be created"
            );
        }
    }
}
