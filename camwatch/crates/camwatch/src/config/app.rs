use std::{net::SocketAddr, path::PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    pub bind_address: SocketAddr,
    pub database_path: PathBuf,
    pub pre_event_seconds: u32,
    pub post_event_seconds: u32,
    pub rolling_buffer_seconds: u32,
    #[serde(default = "default_segment_directory")]
    pub segment_directory: PathBuf,
    #[serde(default = "default_segment_rotation_seconds")]
    pub segment_rotation_seconds: u32,
}

fn default_segment_directory() -> PathBuf {
    PathBuf::from("data/segments")
}

fn default_segment_rotation_seconds() -> u32 {
    2
}
