use crate::clips::Clip;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadRequest {
    pub event_id: String,
    pub clip: Clip,
}
