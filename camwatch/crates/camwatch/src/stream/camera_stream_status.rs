use std::time::SystemTime;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CameraStreamStatus {
    Online { since: SystemTime },
    Offline { since: SystemTime },
}
