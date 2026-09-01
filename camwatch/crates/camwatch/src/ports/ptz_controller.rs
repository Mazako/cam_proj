use super::PortFuture;

pub trait PtzController: Send + Sync {
    fn capabilities(&self) -> PortFuture<'_, Result<PtzCapabilities, PtzControllerError>>;

    fn move_continuously(
        &self,
        movement: PtzMove,
    ) -> PortFuture<'_, Result<(), PtzControllerError>>;

    fn stop(&self) -> PortFuture<'_, Result<(), PtzControllerError>>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PtzCapabilities {
    pub supported: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PtzMove {
    pub direction: PtzDirection,
    pub speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PtzDirection {
    Up(f32),
    Down(f32),
    Left(f32),
    Right(f32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PtzControllerError {
    Unsupported,
    Unavailable,
    Failed,
}
