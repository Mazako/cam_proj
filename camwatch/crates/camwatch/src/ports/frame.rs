use derive_new::new;
use std::time::SystemTime;

use opencv::core::{Mat, Vec3b};

#[derive(Clone, Debug, PartialEq, new)]
pub struct Frame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub captured_at: SystemTime,
}

impl Frame {
    pub fn as_mat(&self) -> Result<opencv::boxed_ref::BoxedRef<'_, Mat>, opencv::Error> {
        Mat::new_rows_cols_with_bytes::<Vec3b>(self.height as i32, self.width as i32, &self.data)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PixelFormat {
    Bgr8,
    Rgb8,
}
