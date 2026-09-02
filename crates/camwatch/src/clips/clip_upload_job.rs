use crate::bucket::UploadRequest;

pub struct ClipUploadJob {
    pub camera_id: String,
    pub request: UploadRequest,
}
