use std::{future::Future, pin::Pin};

use super::{PtzCapabilities, PtzControllerError, PtzMove};

pub type PtzFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait PtzController: Send + Sync {
    fn capabilities(&self) -> PtzFuture<'_, Result<PtzCapabilities, PtzControllerError>>;

    fn move_continuously(&self, movement: PtzMove)
    -> PtzFuture<'_, Result<(), PtzControllerError>>;

    fn stop(&self) -> PtzFuture<'_, Result<(), PtzControllerError>>;
}
