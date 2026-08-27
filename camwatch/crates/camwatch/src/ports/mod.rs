mod camera_stream;
mod drive_uploader;
mod frame;
mod motion_detector;
mod person_detector;
mod ptz_controller;

use std::{future::Future, pin::Pin};

pub use camera_stream::{CameraStream, CameraStreamError, CameraStreamEvent, CameraStreamStatus};
pub use drive_uploader::{DriveUploader, DriveUploaderError, RemoteFile, UploadRequest};
pub use frame::{Frame, PixelFormat};
pub use motion_detector::{Motion, MotionDetector, MotionDetectorError};
pub use person_detector::{BoundingBox, PersonDetection, PersonDetector, PersonDetectorError};
pub use ptz_controller::{
    PtzCapabilities, PtzController, PtzControllerError, PtzDirection, PtzMove,
};

pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
