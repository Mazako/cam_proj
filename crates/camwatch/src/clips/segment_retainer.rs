use std::{sync::Arc, time::Duration};

use super::ClipManager;

pub fn create_retainer_worker(rolling_buffer_seconds: u64, clip_manager: Arc<ClipManager>) {
    tokio::spawn(async move {
        let interval = Duration::from_secs(rolling_buffer_seconds);
        loop {
            clip_manager
                .retain_expired_segments(rolling_buffer_seconds)
                .await;
            tokio::time::sleep(interval).await;
        }
    });
}
