use super::StorageError;

#[derive(Clone, Debug, PartialEq)]
pub struct NewUpload {
    pub event_id: String,
    pub provider: String,
    pub next_attempt_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Upload {
    pub id: String,
    pub event_id: String,
    pub provider: String,
    pub status: UploadStatus,
    pub attempt_count: i64,
    pub next_attempt_at: Option<i64>,
    pub remote_file_id: Option<String>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UploadStatus {
    Pending,
    InProgress,
    Uploaded,
    Failed,
}

impl UploadStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Uploaded => "uploaded",
            Self::Failed => "failed",
        }
    }

    pub(super) fn parse(value: &str) -> Result<Self, StorageError> {
        match value {
            "pending" => Ok(Self::Pending),
            "in_progress" => Ok(Self::InProgress),
            "uploaded" => Ok(Self::Uploaded),
            "failed" => Ok(Self::Failed),
            _ => Err(StorageError::InvalidStoredStatus),
        }
    }
}
