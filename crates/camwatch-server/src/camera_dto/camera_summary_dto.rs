use camwatch::stream::CameraStreamStatus;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraSummaryDto {
    pub id: String,
    pub name: String,
    pub runtime_running: bool,
    pub stream_status: Option<CameraStreamStatus>,
    pub ptz_available: bool,
}
