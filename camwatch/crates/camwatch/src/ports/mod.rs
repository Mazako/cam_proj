mod bucket_uploader;
mod camera_stream;
mod frame;
mod motion_detector;
mod person_detector;
mod ptz_controller;

use std::{future::Future, pin::Pin};

pub use bucket_uploader::{BucketUploader, BucketUploaderError, RemoteObject, UploadRequest};
pub use camera_stream::{CameraStream, CameraStreamError, CameraStreamEvent, CameraStreamStatus};
pub use frame::{Frame, PixelFormat};
pub use motion_detector::{Motion, MotionDetector, MotionDetectorError};
pub use person_detector::{BoundingBox, PersonDetection, PersonDetector, PersonDetectorError};
pub use ptz_controller::{
    PtzCapabilities, PtzController, PtzControllerError, PtzDirection, PtzMove,
};

pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
