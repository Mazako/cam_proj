# Camwatch

Rust application for local network camera monitoring.

## Running

```sh
cargo run -p camwatch -- --config config/camwatch.example.toml
```

The TOML configuration stores only non-sensitive data and names of environment variables that hold secrets, such as `CAMWATCH_FRONT_DOOR_RTSP_URL`. Do not put passwords, tokens, or RTSP URLs with credentials in it.

The default panel address is `127.0.0.1:8080`. To deliberately change the configuration path, pass `--config` or set the `CAMWATCH_CONFIG` environment variable.
