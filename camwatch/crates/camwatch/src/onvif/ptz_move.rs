use super::PtzDirection;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PtzMove {
    pub direction: PtzDirection,
    pub speed: f32,
}
