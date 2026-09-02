use std::{
    io::ErrorKind,
    sync::Arc,
    time::{Duration, SystemTime},
};

use tokio::fs;

use crate::storage::Database;

use super::ClipManager;

pub fn create_retainer_worker(
    db: Database,
    rolling_buffer_seconds: u64,
    clip_manager: Arc<ClipManager>,
) {
    tokio::spawn(async move {
        let interval = Duration::from_secs(rolling_buffer_seconds);
        loop {
            retain_expired_segments(&db, rolling_buffer_seconds, &clip_manager).await;
            tokio::time::sleep(interval).await;
        }
    });
}

pub async fn retain_expired_segments(
    db: &Database,
    rolling_buffer_seconds: u64,
    clip_manager: &ClipManager,
) {
    let Some(before) = SystemTime::now().checked_sub(Duration::from_secs(rolling_buffer_seconds))
    else {
        return;
    };

    let segments = match db.get_segments_finished_before(before).await {
        Ok(segments) => segments,
        Err(error) => {
            tracing::warn!(%error, "failed to load expired segments");
            return;
        }
    };

    let mut removed_paths = Vec::new();
    for segment in segments {
        if clip_manager.is_segment_reserved(&segment.path) {
            continue;
        }

        match fs::remove_file(&segment.path).await {
            Ok(()) => removed_paths.push(segment.path),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                removed_paths.push(segment.path);
            }
            Err(error) => {
                tracing::warn!(
                    path = %segment.path,
                    %error,
                    "failed to remove segment file"
                );
            }
        }
    }

    if !removed_paths.is_empty()
        && let Err(error) = db.remove_segments(&removed_paths).await
    {
        tracing::warn!(
            %error,
            count = removed_paths.len(),
            "failed to remove segments from database"
        );
    }
}
