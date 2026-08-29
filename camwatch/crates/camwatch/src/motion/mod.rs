pub use crate::ports::{Motion, MotionDetector, MotionDetectorError};

mod cv2_helper;
mod mog2;
mod yolo;
pub use mog2::Mog2MotionDetector;
pub use yolo::{YoloAnalyzer, YoloAnalyzerError, Detection, DetectionClass};
