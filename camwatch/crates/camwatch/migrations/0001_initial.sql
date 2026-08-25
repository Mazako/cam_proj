CREATE TABLE IF NOT EXISTS cameras (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    rtsp_url_env TEXT NOT NULL,
    onvif_url TEXT,
    onvif_credentials_env TEXT,
    motion_min_area INTEGER NOT NULL CHECK (motion_min_area > 0),
    yolo_confidence REAL NOT NULL CHECK (yolo_confidence BETWEEN 0.0 AND 1.0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    CHECK (
        (onvif_url IS NULL AND onvif_credentials_env IS NULL)
        OR (onvif_url IS NOT NULL AND onvif_credentials_env IS NOT NULL)
    )
);

CREATE TABLE IF NOT EXISTS "events" (
    id TEXT PRIMARY KEY,
    camera_id TEXT NOT NULL REFERENCES cameras (id),
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    "trigger" TEXT NOT NULL,
    clip_path TEXT,
    clip_duration_ms INTEGER,
    status TEXT NOT NULL CHECK (status IN ('recording', 'finalizing', 'ready', 'failed')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (ended_at IS NULL OR ended_at >= started_at),
    CHECK (clip_duration_ms IS NULL OR clip_duration_ms >= 0)
);

CREATE TABLE IF NOT EXISTS uploads (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL REFERENCES "events" (id),
    provider TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'in_progress', 'uploaded', 'failed')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at INTEGER,
    remote_file_id TEXT,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_camera_started_at ON "events" (camera_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_uploads_status_next_attempt_at ON uploads (status, next_attempt_at);
