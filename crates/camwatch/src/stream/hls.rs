use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

const PLAYLIST_FILE_NAME: &str = "index.m3u8";
const SEGMENT_FILE_PREFIX: &str = "segment-";
const SEGMENT_FILE_SUFFIX: &str = ".ts";

pub(crate) const TARGET_DURATION_SECONDS: u32 = 2;
pub(crate) const PLAYLIST_LENGTH: u32 = 5;
pub(crate) const MAX_FILES: u32 = 6;

#[derive(Clone, Debug)]
pub struct HlsConfig {
    directory: PathBuf,
}

impl HlsConfig {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub(crate) fn output(&self) -> Result<HlsOutput, HlsOutputError> {
        fs::create_dir_all(&self.directory).map_err(HlsOutputError::Directory)?;
        clean_previous_output(&self.directory).map_err(HlsOutputError::Directory)?;

        Ok(HlsOutput {
            playlist_location: self.directory.join(PLAYLIST_FILE_NAME),
            segment_location: self
                .directory
                .join(format!("{SEGMENT_FILE_PREFIX}%05d{SEGMENT_FILE_SUFFIX}")),
        })
    }
}

pub(crate) struct HlsOutput {
    pub(crate) playlist_location: PathBuf,
    pub(crate) segment_location: PathBuf,
}

#[derive(Debug, Error)]
pub enum HlsOutputError {
    #[error("HLS output directory is unavailable")]
    Directory(#[source] io::Error),
}

fn clean_previous_output(directory: &Path) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let is_playlist = file_name == PLAYLIST_FILE_NAME;
        let is_segment =
            file_name.starts_with(SEGMENT_FILE_PREFIX) && file_name.ends_with(SEGMENT_FILE_SUFFIX);
        if is_playlist || is_segment {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
