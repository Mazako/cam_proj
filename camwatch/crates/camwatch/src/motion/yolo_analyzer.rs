use ort::{session::Session, value::TensorRef};

use crate::{motion::cv2_helper::mat_to_yolo_tensor, stream::Frame};

use super::{Detection, DetectionClass, YoloAnalyzerError};

static MODEL_BYTES: &[u8] = include_bytes!("../../../../../yolo26n.onnx");
static PERSON_CLASS: i32 = 0;
static CAT_CLASS: i32 = 15;
static DOG_CLASS: i32 = 16;

pub struct YoloAnalyzer {
    session: Session,
    min_confidence: f32,
}

impl YoloAnalyzer {
    pub fn new(min_confidence: f32) -> Result<Self, ort::Error> {
        let session = Session::builder()?.commit_from_memory(MODEL_BYTES)?;

        Ok(Self {
            session,
            min_confidence,
        })
    }

    pub fn analyze(&mut self, frame: &Frame) -> Result<Vec<Detection>, YoloAnalyzerError> {
        let mat = frame.as_mat()?;
        let input = mat_to_yolo_tensor(&mat)?;
        let mut detections = Vec::new();
        let result = self
            .session
            .run(ort::inputs![TensorRef::from_array_view(&input)?])?;
        for (_, value) in result.iter() {
            if let Ok(tensor) = value.try_extract_tensor::<f32>() {
                tensor.1.as_chunks::<6>().0.iter().for_each(|chunk| {
                    let conf = chunk[4];
                    let class_id = chunk[5] as i32;
                    if (class_id == PERSON_CLASS || class_id == CAT_CLASS || class_id == DOG_CLASS)
                        && conf > self.min_confidence
                    {
                        let detection =
                            Detection::new(DetectionClass::from_class_id(class_id).unwrap(), conf);
                        detections.push(detection);
                    }
                });
            } else {
                eprintln!("Failed to extract tensor from value: {:?}", value);
            }
        }
        Ok(detections)
    }
}
