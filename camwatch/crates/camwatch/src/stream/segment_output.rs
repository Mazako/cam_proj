use std::path::PathBuf;

use derive_new::new;

#[derive(new)]
pub(crate) struct SegmentOutput {
    pub(crate) location: PathBuf,
    pub(crate) rotation_nanoseconds: u64,
    pub(crate) start_index: i32,
}
