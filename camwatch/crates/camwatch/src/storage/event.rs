use super::StorageError;

#[derive(Clone, Debug, PartialEq)]
pub struct NewEvent {
    pub camera_id: String,
    pub started_at: i64,
    pub trigger: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub id: String,
    pub camera_id: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub trigger: String,
    pub clip_path: Option<String>,
    pub clip_duration_ms: Option<i64>,
    pub status: EventStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventStatus {
    Recording,
    Finalizing,
    Ready,
    Failed,
}

impl EventStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Finalizing => "finalizing",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    pub(super) fn parse(value: &str) -> Result<Self, StorageError> {
        match value {
            "recording" => Ok(Self::Recording),
            "finalizing" => Ok(Self::Finalizing),
            "ready" => Ok(Self::Ready),
            "failed" => Ok(Self::Failed),
            _ => Err(StorageError::InvalidStoredStatus),
        }
    }
}
