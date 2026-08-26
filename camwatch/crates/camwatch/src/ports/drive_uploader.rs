use super::{Clip, PortFuture};

pub trait DriveUploader: Send + Sync {
    fn upload(
        &self,
        request: UploadRequest,
    ) -> PortFuture<'_, Result<RemoteFile, DriveUploaderError>>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadRequest {
    pub event_id: String,
    pub clip: Clip,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteFile {
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DriveUploaderError {
    Unavailable,
    Failed,
}
