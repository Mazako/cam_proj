# Camwatch

Rust application for local network camera monitoring.

## Running

```sh
cargo run -p camwatch-server -- --config config/camwatch.example.toml
```

The TOML configuration stores only non-sensitive data and names of environment variables that hold secrets, such as `CAMWATCH_FRONT_DOOR_RTSP_URL`. Do not put passwords, tokens, or RTSP URLs with credentials in it.

The default panel address is `127.0.0.1:8080`. To deliberately change the configuration path, pass `--config` or set the `CAMWATCH_CONFIG` environment variable.

MP4 segments are written below `segment_directory`, in a separate directory for every camera. `segment_rotation_seconds` determines when GStreamer requests the next segment; the actual boundary is the next keyframe of the camera stream.

On the first run, the application creates the SQLite database at `database_path` and seeds it with the cameras from TOML. On subsequent runs, SQLite is the source of truth for cameras and segment metadata.
