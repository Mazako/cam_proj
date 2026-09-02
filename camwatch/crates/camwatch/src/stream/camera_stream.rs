use std::{future::Future, path::PathBuf, pin::Pin, time::SystemTime};

use super::Frame;

pub type CameraStreamFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait CameraStream: Send {
    fn next_event(
        &mut self,
    ) -> CameraStreamFuture<'_, Result<CameraStreamEvent, CameraStreamError>>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum CameraStreamEvent {
    Status(CameraStreamStatus),
    Frame(Frame),
    SegmentFinalized {
        path: PathBuf,
        started_at: SystemTime,
        ended_at: SystemTime,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CameraStreamStatus {
    Online { since: SystemTime },
    Offline { since: SystemTime },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CameraStreamError {
    Unavailable,
    Failed,
}
