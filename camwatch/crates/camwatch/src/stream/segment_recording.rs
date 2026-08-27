use std::{fs, path::PathBuf, time::Duration};

use thiserror::Error;

#[derive(Clone, Debug)]
pub struct SegmentRecordingConfig {
    directory: PathBuf,
    rotation: Duration,
}

impl SegmentRecordingConfig {
    pub fn new(directory: PathBuf, rotation: Duration) -> Self {
        Self {
            directory,
            rotation,
        }
    }

    pub(crate) fn output(&self) -> Result<SegmentOutput, SegmentRecordingError> {
        fs::create_dir_all(&self.directory).map_err(|_| SegmentRecordingError::Directory)?;
        let mut largest_index: Option<i32> = None;
        let entries =
            fs::read_dir(&self.directory).map_err(|_| SegmentRecordingError::Directory)?;

        for entry in entries {
            let entry = entry.map_err(|_| SegmentRecordingError::Directory)?;
            let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(index) = file_name
                .strip_prefix("segment-")
                .and_then(|name| name.strip_suffix(".mp4"))
                .and_then(|index| index.parse::<i32>().ok())
            else {
                continue;
            };
            largest_index = Some(largest_index.map_or(index, |current| current.max(index)));
        }

        let start_index = match largest_index {
            Some(index) => index
                .checked_add(1)
                .ok_or(SegmentRecordingError::IndexExhausted)?,
            None => 0,
        };

        Ok(SegmentOutput {
            location: self.directory.join("segment-%010d.mp4"),
            rotation_nanoseconds: self.rotation.as_nanos().try_into().unwrap_or(u64::MAX),
            start_index,
        })
    }
}

#[derive(Debug, Error)]
pub enum SegmentRecordingError {
    #[error("cannot prepare recording directory")]
    Directory,
    #[error("recording segment index is exhausted")]
    IndexExhausted,
}

pub(crate) struct SegmentOutput {
    pub(crate) location: PathBuf,
    pub(crate) rotation_nanoseconds: u64,
    pub(crate) start_index: i32,
}
