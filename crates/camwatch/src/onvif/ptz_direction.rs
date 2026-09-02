#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PtzDirection {
    Up(f32),
    Down(f32),
    Left(f32),
    Right(f32),
}
