use thiserror::Error;

#[derive(Debug, Error)]
pub enum R2Error {
    #[error("missing R2 environment variable reference: {0}")]
    MissingEnvironmentVariableReference(&'static str),
    #[error("missing R2 environment variable: {0}")]
    MissingEnvironmentVariable(String),
    #[error("invalid R2 endpoint")]
    InvalidEndpoint,
    #[error("invalid R2 configuration")]
    InvalidConfiguration,
    #[error("cannot read clip for R2 upload")]
    ReadClip(String),
    #[error("R2 upload failed")]
    Upload(
        #[source]
        Box<aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::put_object::PutObjectError>>,
    ),
}
