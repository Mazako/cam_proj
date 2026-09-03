CREATE TABLE IF NOT EXISTS cameras (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    rtsp_url_env TEXT NOT NULL,
    rtsp_codec TEXT NOT NULL DEFAULT 'h264' CHECK (rtsp_codec IN ('h264', 'h265')),
    onvif_url TEXT,
    onvif_credentials_env TEXT,
    motion_min_area INTEGER NOT NULL CHECK (motion_min_area > 0),
    yolo_confidence REAL NOT NULL CHECK (yolo_confidence BETWEEN 0.0 AND 1.0),
    clip_after_motion INTEGER NOT NULL DEFAULT 1 CHECK (clip_after_motion IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    CHECK (
        (onvif_url IS NULL AND onvif_credentials_env IS NULL)
        OR (onvif_url IS NOT NULL AND onvif_credentials_env IS NOT NULL)
    )
);

CREATE TABLE IF NOT EXISTS segments (
    path TEXT PRIMARY KEY,
    camera_id TEXT NOT NULL REFERENCES cameras (id),
    started_at INTEGER NOT NULL,
    ended_at INTEGER NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (ended_at >= started_at)
);

CREATE INDEX IF NOT EXISTS idx_segments_camera_time ON segments (camera_id, started_at, ended_at);
