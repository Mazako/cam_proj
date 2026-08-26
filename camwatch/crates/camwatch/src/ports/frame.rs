use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub captured_at: SystemTime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PixelFormat {
    Bgr8,
    Rgb8,
}
