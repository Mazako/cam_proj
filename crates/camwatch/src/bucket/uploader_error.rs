use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BucketUploaderError {
    #[error("bucket is unavailable")]
    Unavailable,
    #[error("bucket upload failed")]
    Failed,
}
