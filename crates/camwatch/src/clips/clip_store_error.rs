use thiserror::Error;

use crate::storage::StorageError;

#[derive(Debug, Error)]
pub enum ClipStoreError {
    #[error("segment time range is invalid")]
    InvalidTimeRange,
    #[error("path is not valid UTF-8")]
    InvalidPath,
    #[error("no segments overlap the requested clip range")]
    NoSegments,
    #[error("cannot read file metadata")]
    FileMetadata(#[source] std::io::Error),
    #[error("cannot create clip directory")]
    CreateDirectory(#[source] std::io::Error),
    #[error("cannot stage segment")]
    StageSegment(#[source] std::io::Error),
    #[error("cannot create temporary clip directory")]
    TemporaryDirectory(#[source] std::io::Error),
    #[error("GStreamer could not initialize")]
    GstreamerInitialization,
    #[error("GStreamer pipeline could not be built")]
    PipelineBuild,
    #[error("GStreamer pipeline could not start")]
    PipelineStart,
    #[error("GStreamer pipeline failed while creating the clip")]
    PipelineExecution,
    #[error("clip metadata could not be read")]
    ClipMetadata,
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("clip assembly task stopped unexpectedly")]
    AssemblyTask,
}
