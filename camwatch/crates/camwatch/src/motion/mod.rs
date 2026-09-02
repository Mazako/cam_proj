mod motion_detector;
mod person_detector;

mod cv2_helper;
mod mog2;
mod yolo;
pub use mog2::Mog2MotionDetector;
pub use motion_detector::{Motion, MotionDetector, MotionDetectorError};
pub use person_detector::{BoundingBox, PersonDetection, PersonDetector, PersonDetectorError};
pub use yolo::{Detection, DetectionClass, YoloAnalyzer, YoloAnalyzerError};
