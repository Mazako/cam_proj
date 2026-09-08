use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClipStoreError {
    #[error("segment time range is invalid")]
    InvalidTimeRange,
    #[error("segment is already registered")]
    SegmentAlreadyRegistered,
    #[error("path cannot be converted to a file URL")]
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
    #[error("clip assembly task stopped unexpectedly")]
    AssemblyTask,
}
