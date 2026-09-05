use thiserror::Error;

use super::{SegmentRecordingError, hls::HlsOutputError};

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error(transparent)]
    Recording(#[from] SegmentRecordingError),
    #[error(transparent)]
    Hls(#[from] HlsOutputError),
    #[error("GStreamer pipeline could not be built")]
    Build,
}
