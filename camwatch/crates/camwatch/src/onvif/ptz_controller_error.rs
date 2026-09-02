#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PtzControllerError {
    Unsupported,
    Unavailable,
    Failed,
}
