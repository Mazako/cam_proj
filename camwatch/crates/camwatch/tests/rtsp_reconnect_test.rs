use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, SystemTime},
};

use camwatch::{
    clips::{create_clip, store_segment},
    ports::{CameraStream, CameraStreamEvent, CameraStreamStatus},
    storage::{Database, NewCamera},
    stream::{GstreamerCameraStream, RtspCodec, SegmentRecordingConfig},
};
use tempfile::tempdir;
use testcontainers::{GenericImage, ImageExt, core::IntoContainerPort, runners::AsyncRunner};

struct TestPublisher {
    child: Child,
}

impl TestPublisher {
    fn start(rtsp_url: &str) -> Self {
        let child = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-re",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x240:rate=10",
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

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for TestPublisher {
    fn drop(&mut self) {
        self.stop();
    }
}

#[tokio::test]
async fn returns_online_after_the_rtsp_publisher_restarts() {
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
    let rtsp_url = format!("rtsp://127.0.0.1:{port}/test");
    let mut publisher = TestPublisher::start(&rtsp_url);
    let directory = tempdir().expect("temporary directory should exist");
    let recording =
        SegmentRecordingConfig::new(directory.path().to_path_buf(), Duration::from_secs(1));
    let mut stream = GstreamerCameraStream::new(rtsp_url.clone(), RtspCodec::H264, recording)
        .expect("RTSP stream should start");

    wait_for_online_frame(&mut stream).await;

    publisher.stop();
    wait_for_offline(&mut stream).await;

    let _publisher = TestPublisher::start(&rtsp_url);
    wait_for_online_frame(&mut stream).await;
}

#[tokio::test]
async fn writes_playable_mp4_segments_from_the_rtsp_stream() {
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
    let rtsp_url = format!("rtsp://127.0.0.1:{port}/recording-test");
    let _publisher = TestPublisher::start(&rtsp_url);
    let directory = tempdir().expect("temporary directory should exist");
    let recording =
        SegmentRecordingConfig::new(directory.path().to_path_buf(), Duration::from_secs(1));
    let mut stream = GstreamerCameraStream::new(rtsp_url, RtspCodec::H264, recording)
        .expect("RTSP recording stream should start");

    wait_for_online_frame(&mut stream).await;
    let (segment, _, _) = wait_for_finalized_segment(&mut stream).await;

    assert!(segment.starts_with(directory.path()));
    assert!(is_playable_mp4(&segment));
}

#[tokio::test]
async fn assembles_a_clip_from_persisted_rtsp_segments() {
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
    let rtsp_url = format!("rtsp://127.0.0.1:{port}/clip-test");
    let _publisher = TestPublisher::start(&rtsp_url);
    let directory = tempdir().expect("temporary directory should exist");
    let recording =
        SegmentRecordingConfig::new(directory.path().join("segments"), Duration::from_secs(1));
    let mut stream = GstreamerCameraStream::new(rtsp_url, RtspCodec::H264, recording)
        .expect("RTSP recording stream should start");
    let database = database_with_camera(directory.path()).await;

    wait_for_online_frame(&mut stream).await;
    let first = wait_for_finalized_segment(&mut stream).await;
    let second = wait_for_finalized_segment(&mut stream).await;

    for (path, started_at, ended_at) in [&first, &second] {
        store_segment(
            &database,
            "front-door",
            path.clone(),
            *started_at,
            *ended_at,
        )
        .await
        .expect("finalized segment should be stored");
    }

    let clip = create_clip(
        &database,
        "front-door",
        first.1,
        second.2,
        directory.path().join("clips/event-1.mp4"),
    )
    .await
    .expect("clip should be assembled");

    assert!(clip.path.is_file());
    assert!(clip.duration > Duration::ZERO);
    assert!(is_playable_mp4(&clip.path));
}

async fn wait_for_rtsp_server(port: u16) {
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

async fn wait_for_online_frame(stream: &mut GstreamerCameraStream) {
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

async fn wait_for_offline(stream: &mut GstreamerCameraStream) {
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

async fn wait_for_finalized_segment(
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

async fn database_with_camera(directory: &Path) -> Database {
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

fn is_playable_mp4(path: &Path) -> bool {
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
