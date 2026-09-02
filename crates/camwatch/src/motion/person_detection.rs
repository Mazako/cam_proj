use super::BoundingBox;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PersonDetection {
    pub bounding_box: BoundingBox,
    pub confidence: f32,
}
