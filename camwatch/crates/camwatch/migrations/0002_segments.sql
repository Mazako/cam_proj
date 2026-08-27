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
