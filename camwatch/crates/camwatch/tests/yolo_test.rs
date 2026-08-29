use camwatch::{motion::{DetectionClass, YoloAnalyzer}, ports::Frame};
use opencv::{core::{MatTraitConst, MatTraitConstManual}, imgcodecs::imread};

#[test]
pub fn should_detect_person() {
    let frame = load_frame("tests/resources/image.png");
    let detections = YoloAnalyzer::new(0.5).unwrap().analyze(&frame).unwrap();
    assert_eq!(detections.len(), 1);
    let detection = &detections[0];
    assert_eq!(detection.class, DetectionClass::Person);
    assert!(detection.confidence > 0.5);
}

#[test]
pub fn should_detect_cat() {
    let frame = load_frame("tests/resources/cat.png");
    let detections = YoloAnalyzer::new(0.3).unwrap().analyze(&frame).unwrap();
    assert!(detections.iter().any(|d| d.class == DetectionClass::Cat));
    assert!(detections.iter().any(|d| d.confidence >= 0.3));
    assert!(!detections.iter().any(|d| d.confidence < 0.3));
}


pub fn load_frame(path: &str) -> Frame {
    let img = imread(path, opencv::imgcodecs::IMREAD_COLOR).unwrap();
    let size = img.size().unwrap();
    let width = size.width as u32;
    let height = size.height as u32;
    let data = img.data_bytes().unwrap();
    Frame::new(data.to_vec(), width, height, camwatch::ports::PixelFormat::Bgr8, std::time::SystemTime::now())
}