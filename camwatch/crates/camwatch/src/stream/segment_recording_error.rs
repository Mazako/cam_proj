use thiserror::Error;

#[derive(Debug, Error)]
pub enum SegmentRecordingError {
    #[error("cannot prepare recording directory")]
    Directory,
    #[error("recording segment index is exhausted")]
    IndexExhausted,
}
