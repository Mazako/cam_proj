use std::{future::Future, pin::Pin};

use crate::stream::Frame;

pub type PersonDetectorFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait PersonDetector: Send + Sync {
    fn detect(
        &self,
        frame: &Frame,
    ) -> PersonDetectorFuture<'_, Result<Vec<PersonDetection>, PersonDetectorError>>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PersonDetection {
    pub bounding_box: BoundingBox,
    pub confidence: f32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersonDetectorError {
    Failed,
}
