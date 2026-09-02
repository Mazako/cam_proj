use thiserror::Error;

use super::SegmentRecordingError;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error(transparent)]
    Recording(#[from] SegmentRecordingError),
    #[error("GStreamer pipeline could not be built")]
    Build,
}
