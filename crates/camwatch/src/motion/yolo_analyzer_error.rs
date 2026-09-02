#[derive(Debug, thiserror::Error)]
pub enum YoloAnalyzerError {
    #[error("OpenCV error: {0}")]
    OpenCv(#[from] opencv::Error),
    #[error("ONNX Runtime error: {0}")]
    Ort(#[from] ort::Error),
}
