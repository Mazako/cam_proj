use std::{future::Future, pin::Pin};

use crate::stream::Frame;

use super::{PersonDetection, PersonDetectorError};

pub type PersonDetectorFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait PersonDetector: Send + Sync {
    fn detect(
        &self,
        frame: &Frame,
    ) -> PersonDetectorFuture<'_, Result<Vec<PersonDetection>, PersonDetectorError>>;
}
