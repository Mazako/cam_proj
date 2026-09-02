mod camera_stream;
mod codec;
mod frame;
mod gstreamer;
mod pipeline;
mod segment_recording;
mod status_model;

pub use camera_stream::{
    CameraStream, CameraStreamError, CameraStreamEvent, CameraStreamFuture, CameraStreamStatus,
};
pub use codec::RtspCodec;
pub use frame::{Frame, PixelFormat};
pub use gstreamer::GstreamerCameraStream;
pub use pipeline::{PipelineError, build_pipeline};
pub use segment_recording::{SegmentRecordingConfig, SegmentRecordingError};
pub use status_model::CameraStatusModel;

pub(crate) fn escape_pipeline_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
