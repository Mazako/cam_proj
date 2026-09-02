use std::{
    env,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, SystemTime},
};

use camwatch::{
    storage::{Database, NewCamera},
    stream::{
        CameraStream, CameraStreamEvent, CameraStreamStatus, GstreamerCameraStream, RtspCodec,
        SegmentRecordingConfig,
    },
};
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt, core::IntoContainerPort, runners::AsyncRunner,
};

pub const BACKGROUND_LEARNING_FRAMES: usize = 90;
pub const MIN_MOTION_AREA: f64 = 1_000.0;
pub const PETS2006_ENCODED_FRAMES: usize = 400;

pub struct RtspSession {
    _server: ContainerAsync<GenericImage>,
    pub url: String,
    pub publisher: TestPublisher,
}

impl RtspSession {
    pub async fn start(path: &str, video_path: Option<&Path>) -> Self {
        let server = GenericImage::new("bluenviron/mediamtx", "1")
            .with_exposed_port(8554.tcp())
            .with_env_var("MTX_RTSPTRANSPORTS", "tcp")
            .start()
            .await
            .expect("MediaMTX container should start");
        let port = server
            .get_host_port_ipv4(8554)
            .await
            .expect("MediaMTX RTSP port should be available");
        wait_for_rtsp_server(port).await;
        let url = format!("rtsp://127.0.0.1:{port}/{path}");
        let publisher = TestPublisher::start(&url, video_path);
        Self {
            _server: server,
            url,
            publisher,
        }
    }
}

pub struct TestPublisher {
    child: Child,
}

impl TestPublisher {
    pub fn start(rtsp_url: &str, video_path: Option<&Path>) -> Self {
        let mut command = Command::new("ffmpeg");
        command.args(["-hide_banner", "-loglevel", "error", "-re"]);
        match video_path {
            Some(video) => {
                command.args([
                    "-stream_loop",
                    "-1",
                    "-i",
                    video.to_str().expect("video path should be UTF-8"),
                ]);
            }
            None => {
                command.args(["-f", "lavfi", "-i", "testsrc2=size=320x240:rate=10"]);
            }
        }
        let child = command
            .args([
                "-an",
                "-c:v",
                "libx264",
                "-g",
                "10",
                "-keyint_min",
                "10",
                "-preset",
                "ultrafast",
                "-tune",
                "zerolatency",
                "-f",
                "rtsp",
                "-rtsp_transport",
                "tcp",
                rtsp_url,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("FFmpeg must be installed to run this test");

        Self { child }
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for TestPublisher {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn camera_stream(url: String, segments: &Path) -> GstreamerCameraStream {
    let recording = SegmentRecordingConfig::new(segments.to_path_buf(), Duration::from_secs(1));
    GstreamerCameraStream::new(url, RtspCodec::H264, recording).expect("RTSP stream should start")
}

pub async fn wait_for_rtsp_server(port: u16) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                return;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("MediaMTX RTSP listener should become available");
}

pub async fn wait_for_online_frame(stream: &mut GstreamerCameraStream) {
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut online = false;

        loop {
            match stream
                .next_event()
                .await
                .expect("stream should stay available")
            {
                CameraStreamEvent::Status(CameraStreamStatus::Online { .. }) => online = true,
                CameraStreamEvent::Frame(_) if online => return,
                CameraStreamEvent::Status(CameraStreamStatus::Offline { .. })
                | CameraStreamEvent::Frame(_)
                | CameraStreamEvent::SegmentFinalized { .. } => {}
            }
        }
    })
    .await
    .expect("stream should become online and emit a frame");
}

pub async fn wait_for_offline(stream: &mut GstreamerCameraStream) {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if matches!(
                stream
                    .next_event()
                    .await
                    .expect("stream should stay available"),
                CameraStreamEvent::Status(CameraStreamStatus::Offline { .. })
            ) {
                return;
            }
        }
    })
    .await
    .expect("stream should become offline after the publisher stops");
}

pub async fn wait_for_finalized_segment(
    stream: &mut GstreamerCameraStream,
) -> (PathBuf, SystemTime, SystemTime) {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            match stream
                .next_event()
                .await
                .expect("stream should stay available")
            {
                CameraStreamEvent::SegmentFinalized {
                    path,
                    started_at,
                    ended_at,
                } => {
                    assert!(ended_at >= started_at);
                    return (path, started_at, ended_at);
                }
                CameraStreamEvent::Status(_) | CameraStreamEvent::Frame(_) => {}
            }
        }
    })
    .await
    .expect("RTSP stream should report a finalized MP4 segment")
}

pub async fn database_with_camera(directory: &Path) -> Database {
    let (database, _) = Database::open(&directory.join("camwatch.sqlite3"))
        .await
        .expect("database should open");
    database
        .seed_cameras(&[NewCamera {
            id: "front-door".to_owned(),
            name: "Front door".to_owned(),
            rtsp_url_env: "CAMWATCH_FRONT_DOOR_RTSP_URL".to_owned(),
            onvif_url: None,
            onvif_credentials_env: None,
            motion_min_area: 1000,
            yolo_confidence: 0.5,
        }])
        .await
        .expect("camera should be seeded");

    database
}

pub fn pets2006_dataset() -> PathBuf {
    if let Some(path) = env::var_os("CAMWATCH_PETS2006") {
        let dataset = PathBuf::from(path);
        assert!(
            dataset.join("input").is_dir(),
            "CAMWATCH_PETS2006 must point to a PETS2006 dataset with an input directory"
        );
        return dataset;
    }

    let dataset = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/PETS2006");
    assert!(
        dataset.join("input").is_dir(),
        "PETS2006 dataset is required; put it in tests/resources/PETS2006 or set CAMWATCH_PETS2006"
    );
    dataset
}

pub fn assemble_pets2006_mp4(dataset: &Path, output: &Path) {
    let input = dataset.join("input").join("in%06d.jpg");
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-framerate",
            "10",
            "-start_number",
            "1",
            "-i",
            input.to_str().expect("PETS2006 paths should be UTF-8"),
            "-frames:v",
            &PETS2006_ENCODED_FRAMES.to_string(),
            "-an",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-g",
            "10",
            "-keyint_min",
            "10",
            "-preset",
            "ultrafast",
            "-tune",
            "zerolatency",
            output.to_str().expect("output path should be UTF-8"),
        ])
        .status()
        .expect("FFmpeg must be installed to assemble PETS2006");
    assert!(
        status.success(),
        "FFmpeg should assemble PETS2006 frames into {}",
        output.display()
    );
}

pub fn is_playable_mp4(path: &Path) -> bool {
    Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=format_name",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .split(',')
                    .any(|format| format == "mp4")
        })
}
