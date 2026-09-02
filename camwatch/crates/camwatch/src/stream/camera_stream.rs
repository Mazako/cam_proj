use std::{future::Future, pin::Pin};

use super::{CameraStreamError, CameraStreamEvent};

pub type CameraStreamFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait CameraStream: Send {
    fn next_event(
        &mut self,
    ) -> CameraStreamFuture<'_, Result<CameraStreamEvent, CameraStreamError>>;
}
