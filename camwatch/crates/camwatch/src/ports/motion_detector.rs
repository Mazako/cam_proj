use derive_new::new;

use super::Frame;

pub trait MotionDetector: Send {
    fn detect(&mut self, frame: &Frame) -> Result<Motion, MotionDetectorError>;

    fn reset(&mut self) -> Result<(), MotionDetectorError>;
}

#[derive(Clone, Copy, Debug, PartialEq, new)]
pub struct Motion {
    pub largest_contour_area: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MotionDetectorError {
    Failed,
}
