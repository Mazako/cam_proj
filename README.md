# Camwatch

**Camwatch** is a local-first Rust application for monitoring IP cameras. It ingests RTSP streams, keeps a rotating on-disk recording buffer, detects motion, optionally confirms it with an embedded YOLO model, creates event clips, and can upload them to Cloudflare R2. A small server-side-rendered web panel provides camera management, live HLS playback, and ONVIF PTZ controls.

The application is deliberately designed for a trusted local network. Its web server accepts loopback addresses only, so it is not an Internet-facing camera service.

## Highlights

- Multi-camera RTSP / RTSPS monitoring from encrypted TOML configuration or the web panel.
- GStreamer RTSP/TCP ingest, rotating playable MP4 segments, and HLS output.
- OpenCV MOG2 motion detection plus optional embedded YOLO confirmation for people, cats, and dogs.
- MP4 event clips built from configurable pre-event and post-event windows.
- SQLite metadata storage with WAL and foreign-key support.
- Safe rolling-buffer retention that does not remove segments reserved by an active clip.
- Cloudflare R2 uploads through an S3-compatible adapter, retried up to three times without deleting a failed local clip.
- Authenticated SSR camera CRUD, camera status, HLS playback, and ONVIF PTZ controls.
- AES-256-GCM encryption for RTSP URLs, ONVIF credentials, and R2 settings at rest.

## Architecture

```mermaid
flowchart LR
    Camera["IP camera<br/>RTSP / ONVIF"] --> Ingest["GStreamer RTSP ingest<br/>TCP + H.264"]
    Ingest --> Segments["Rotating MP4 segments<br/>on local disk"]
    Ingest --> Frames["Analysis frames"]
    Frames --> Motion["OpenCV MOG2"]
    Motion -->|"motion"| Decision{"clip_after_motion?"}
    Decision -->|"true"| Clip["In-memory clip lifecycle"]
    Decision -->|"false"| YOLO["ONNX Runtime<br/>YOLO"]
    YOLO -->|"person / cat / dog"| Clip
    Segments --> Clip
    Clip --> EventMP4["Event MP4<br/>pre + post window"]
    Clip --> SQLite[("SQLite<br/>cameras + segments")]
    EventMP4 --> Upload["Background uploader<br/>three attempts"]
    Upload --> R2["Cloudflare R2"]
    Ingest --> HLS["HLS playlist + TS segments"]
    HLS --> Panel["Axum + Askama<br/>local web panel"]
    Browser["Local browser"] --> Panel
    Panel --> PTZ["ONVIF PTZ"]
    PTZ --> Camera
```

Camwatch is one modular process, not a microservice system.

| Package | Responsibility |
| --- | --- |
| `crates/camwatch` | Core configuration, encryption, storage, camera runtime, GStreamer, OpenCV, ONNX, ONVIF, clip lifecycle, and R2 adapter. |
| `crates/camwatch-server` | Axum/Askama SSR panel, sessions, CSRF protection, HLS routes, camera CRUD, and runtime reloads. |
| `crates/camwatch-secret` | A deliberately separate CLI that encrypts a single plaintext value for TOML. |

## Event lifecycle

1. Every active camera has its own RTSP runtime. GStreamer records short MP4 segments and exposes frames to the analysis path.
2. MOG2 evaluates frames for motion; `motion_min_area` filters the contour area.
3. With `clip_after_motion = true` (the default), qualifying motion starts the event. With `false`, YOLO must also detect a person, cat, or dog.
4. Camwatch reserves already-written pre-event segments, then collects segments through the post-event window.
5. A background worker assembles those segments into an MP4 below `clips_directory`.
6. Another worker uploads the clip to R2 or performs a no-op when R2 is disabled. A failure is retried three times and never removes the local MP4.
7. The retention worker removes expired, unreserved segments from disk and SQLite. Clip and upload state is intentionally in memory, so a restart cancels unfinished work.

## Requirements

- Rust **1.98.0**; the repository includes `rust-toolchain.toml`.
- Native development packages for GStreamer, its base/good/bad/libav plugins, FFmpeg, OpenCV, Clang, and libclang.
- An H.264 RTSP camera, or Docker for the included synthetic camera.
- Optional: an ONVIF-capable camera for PTZ, and a dedicated Cloudflare R2 bucket for uploads.

The `yolo26n.onnx` model is embedded in the core crate at build time; no separate model download is required.

### Ubuntu dependencies

```sh
sudo apt-get update
sudo apt-get install -y \
  ffmpeg libopencv-dev clang libclang-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad gstreamer1.0-libav
```

On macOS, install the equivalent GStreamer, OpenCV, FFmpeg, Clang, and `pkg-config` packages, then ensure Cargo can discover OpenCV 4 metadata.

## Quick start with the fake camera

The included Compose setup runs MediaMTX and FFmpeg, publishing a 640x360, 10 FPS H.264 test stream. It lets you exercise the local stack without a physical camera.

```sh
docker compose up -d
mkdir -p .local
./scripts/generate-secret-key.sh .local/camwatch.key
```

Encrypt the RTSP URL and copy the printed ciphertext into `rtsp_url` in a local copy of the fake-camera config:

```sh
printf '%s' 'rtsp://127.0.0.1:8554/fake-camera' \
  | ./scripts/encrypt-secret.sh --key .local/camwatch.key

cp config/camwatch.fake-camera.toml config/camwatch-local.toml
```

Start Camwatch with that same key:

```sh
export CAMWATCH_CONFIG_KEY="$(tr -d '\r\n' < .local/camwatch.key)"
cargo run -p camwatch-server -- --config config/camwatch-local.toml
```

Open <http://127.0.0.1:8080>. Unless both login variables below are set, the development credentials are `admin` / `admin`.

```sh
docker compose down
```

See [the fake camera guide](docs/fake-camera.md) for details.

## Configuration

The server reads `camwatch.toml` by default. Supply `--config` or `CAMWATCH_CONFIG` to use another path:

```sh
cargo run -p camwatch-server -- --config config/camwatch-local.toml
CAMWATCH_CONFIG=config/camwatch-local.toml cargo run -p camwatch-server
```

Start from [config/camwatch.example.toml](config/camwatch.example.toml). Unknown keys, duplicate camera IDs, and invalid values are rejected.

### `[app]`

| Key | Required | Default | Meaning |
| --- | --- | --- | --- |
| `bind_address` | Yes | — | HTTP address. Only `127.0.0.1:<port>` or `[::1]:<port>` is accepted; LAN and public binds are rejected. |
| `database_path` | Yes | — | SQLite file location. Parent directories are created automatically. |
| `pre_event_seconds` | Yes | — | Seconds of earlier segments included in an event; it cannot exceed `rolling_buffer_seconds`. |
| `post_event_seconds` | Yes | — | Seconds captured after the trigger; must be positive. |
| `rolling_buffer_seconds` | Yes | — | Segment retention window; must be positive. |
| `segment_directory` | No | `data/segments` | Root directory for per-camera rotating MP4 buffers. |
| `clips_directory` | No | `data/clips` | Root directory for assembled event MP4 files. |
| `hls_directory` | No | `data/hls` | Root directory for per-camera HLS playlists and TS segments. |
| `segment_rotation_seconds` | No | `2` | Requested segment duration; must be positive and no longer than the rolling buffer. Actual rotation happens on a keyframe. |
| `r2_enabled` | No | `false` | Enables R2. When false, Camwatch does not decrypt or validate R2 fields. |
| `r2_endpoint` | With R2 | — | Encrypted S3-compatible R2 endpoint. |
| `r2_access_key_id` | With R2 | — | Encrypted R2 access key ID. |
| `r2_secret_access_key` | With R2 | — | Encrypted R2 secret access key. |
| `r2_bucket` | With R2 | — | Encrypted destination bucket name. |
| `r2_prefix` | No | empty | Encrypted optional prefix prepended to `{event_id}.mp4`; include a trailing `/` yourself when needed. |
| `r2_region` | No | `auto` | Encrypted optional R2 region. |

### `[[cameras]]`

| Key | Required | Default | Meaning |
| --- | --- | --- | --- |
| `id` | Yes | — | Stable identifier containing lowercase letters, digits, and hyphens only. It becomes part of data paths and URLs. |
| `name` | Yes | — | Human-readable camera name. |
| `rtsp_url` | Yes | — | Encrypted `rtsp://` or `rtsps://` URL. |
| `onvif_url` | No | — | HTTP(S) ONVIF device-service URL without embedded credentials; it must be paired with `onvif_credentials`. |
| `onvif_credentials` | No | — | Encrypted ONVIF credential string; it must be paired with `onvif_url`. |
| `motion_min_area` | Yes | — | Minimum MOG2 contour area that counts as motion. |
| `yolo_confidence` | Yes | — | YOLO confidence threshold from `0.0` through `1.0`. |
| `clip_after_motion` | No | `true` | `true`: motion is sufficient. `false`: YOLO must confirm motion before a clip starts. |

The initial TOML file seeds and updates cameras in SQLite at startup. SQLite then remains the durable source of truth for cameras and segment metadata. The panel can add, edit, and soft-delete cameras, applying runtime reloads immediately.

## Secrets and environment variables

Camwatch does not treat TOML as a safe place for plaintext secrets. Encrypted values have this format:

```text
enc:v1:aes256gcm:<base64(nonce || ciphertext || tag)>
```

`CAMWATCH_CONFIG_KEY` is the Base64-encoded 32-byte AES key. Keep it outside the repository and TOML; the supplied scripts create and consume a local key file.

| Variable | Required | Purpose |
| --- | --- | --- |
| `CAMWATCH_CONFIG_KEY` | Yes | Base64-encoded 32-byte key for encrypted configuration and stored camera secrets. |
| `CAMWATCH_CONFIG` | No | Configuration path, equivalent to `--config`. |
| `CAMWATCH_USER_LOGIN` | Together with password | Local panel login name. |
| `CAMWATCH_USER_PASSWORD` | Together with login | Local panel password. |
| `RUST_LOG` | No | Standard tracing filter; defaults to `info`. |

Authentication configuration is strict:

- with neither login variable set, local development uses `admin` / `admin`;
- with both non-empty variables set, those exact values are used;
- a missing counterpart or an empty value stops startup.

Use explicit credentials outside a throwaway local setup:

```sh
export CAMWATCH_USER_LOGIN='camwatch-admin'
export CAMWATCH_USER_PASSWORD='use-a-unique-long-password'
```

Pass values through standard input to avoid writing a secret into shell history:

```sh
printf '%s' 'rtsp://user:password@camera.lan:554/stream1' \
  | ./scripts/encrypt-secret.sh --key .local/camwatch.key
```

Read [docs/secrets.md](docs/secrets.md) for the ciphertext format and further examples.

## Web panel and security model

The panel is server-side rendered with Axum and Askama. It is a local operator interface, not a public API.

- `GET /health` is public and returns `ok`.
- `/login` is public; camera, HLS, PTZ, and logout routes require an active session.
- Cookies are `HttpOnly`, `SameSite=Strict`, scoped to `/`, and expire after one hour of inactivity. Sessions are in memory, so a restart signs every user out.
- Successful login rotates the session ID. Every state-changing form validates a server-side CSRF token.
- Credential comparisons use constant-time equality; failed login attempts have a short delay.
- Non-loopback bind addresses are rejected. Remote access requires a consciously designed, secure reverse proxy or tunnel, which this repository does not provide.

HLS playlists and segments are protected routes. Browsers use native HLS when available and the bundled HLS player fallback otherwise.

## Cloudflare R2

Set `r2_enabled = true` and provide encrypted `r2_endpoint`, `r2_access_key_id`, `r2_secret_access_key`, and `r2_bucket`. R2 uses the S3-compatible AWS SDK and writes objects as:

```text
{r2_prefix}{event_id}.mp4
```

Use a dedicated bucket or prefix and least-privilege credentials. R2 upload work is in memory rather than a durable delivery queue; the local event file survives all failed attempts.

The real R2 test is intentionally ignored during ordinary local tests because it writes to a real bucket. CI only runs it after manual dispatch with `CAMWATCH_R2_*` repository secrets.

## Development and verification

Run the usual repository checks from the root:

```sh
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

The test suite covers configuration and encryption, SQLite storage, camera runtime behavior, segment retention, overlapping event clips, upload retries, SSR authentication and CSRF, HLS serving, camera CRUD, and mocked ONVIF/PTZ behavior. It also contains RTSP, assembly, and R2 integration cases.

Passing local checks is not proof of deployment readiness. Validate the actual camera's RTSP codec and keyframes, ONVIF support, network latency, storage capacity, and R2 permissions in the target environment.

## Repository layout

```text
.
├── config/                         Example and local camera configuration
├── crates/
│   ├── camwatch/                   Core runtime and infrastructure adapters
│   ├── camwatch-server/            Local SSR control panel
│   └── camwatch-secret/            Secret-encryption CLI
├── docs/                           Design notes and operating guides
├── scripts/                        Key generation and encryption helpers
├── docker-compose.yml              MediaMTX + FFmpeg synthetic camera
├── yolo26n.onnx                    Embedded ONNX model
└── Cargo.toml                      Rust workspace
```

## Further reading

- [Project guide and scope](docs/README.md)
- [Fake camera setup](docs/fake-camera.md)
- [Encrypted configuration](docs/secrets.md)
- [SSR backend details](docs/backend-ssr.md)
- [Implementation backlog](docs/tasks.md)

## License

The workspace declares the MIT license.
