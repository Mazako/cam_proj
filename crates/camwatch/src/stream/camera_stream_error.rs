#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CameraStreamError {
    Unavailable,
    Failed,
}
