use std::{future::Future, pin::Pin};

pub type PtzFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait PtzController: Send + Sync {
    fn capabilities(&self) -> PtzFuture<'_, Result<PtzCapabilities, PtzControllerError>>;

    fn move_continuously(&self, movement: PtzMove)
    -> PtzFuture<'_, Result<(), PtzControllerError>>;

    fn stop(&self) -> PtzFuture<'_, Result<(), PtzControllerError>>;
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
