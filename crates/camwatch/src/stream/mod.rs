mod camera_stream;
mod camera_stream_error;
mod camera_stream_event;
mod camera_stream_status;
mod codec;
mod frame;
mod gstreamer;
mod gstreamer_camera_stream;
mod pipeline;
mod pipeline_error;
mod pixel_format;
mod segment_output;
mod segment_recording;
mod segment_recording_error;
mod segment_times;
mod status_model;

pub use camera_stream::{CameraStream, CameraStreamFuture};
pub use camera_stream_error::CameraStreamError;
pub use camera_stream_event::CameraStreamEvent;
pub use camera_stream_status::CameraStreamStatus;
pub use codec::RtspCodec;
pub use frame::Frame;
pub use gstreamer_camera_stream::GstreamerCameraStream;
pub use pipeline::build_pipeline;
pub use pipeline_error::PipelineError;
pub use pixel_format::PixelFormat;
pub use segment_recording::SegmentRecordingConfig;
pub use segment_recording_error::SegmentRecordingError;
pub use status_model::CameraStatusModel;

pub(crate) fn escape_pipeline_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
