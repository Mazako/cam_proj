use crate::stream::Frame;

use super::{Motion, MotionDetectorError};

pub trait MotionDetector: Send {
    fn detect(&mut self, frame: &Frame) -> Result<Motion, MotionDetectorError>;

    fn reset(&mut self) -> Result<(), MotionDetectorError>;
}
