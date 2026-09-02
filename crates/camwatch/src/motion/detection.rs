use derive_new::new;

use super::DetectionClass;

#[derive(new, Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    pub class: DetectionClass,
    pub confidence: f32,
}
