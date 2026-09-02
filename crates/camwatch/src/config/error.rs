use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read file {path}: {source}")]
    Read {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("invalid configuration")]
    InvalidToml,
    #[error("invalid configuration: {0}")]
    Validation(&'static str),
}
