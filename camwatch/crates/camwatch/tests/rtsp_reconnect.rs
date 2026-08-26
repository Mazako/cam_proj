use std::{
    process::{Child, Command, Stdio},
    time::Duration,
};

use camwatch::{
    ports::{CameraStream, CameraStreamEvent, CameraStreamStatus},
    stream::{GstreamerCameraStream, RtspCodec},
};
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
    let mut stream = GstreamerCameraStream::new(rtsp_url.clone(), RtspCodec::H264)
        .expect("RTSP stream should start");

    wait_for_online_frame(&mut stream).await;

    publisher.stop();
    wait_for_offline(&mut stream).await;

    let _publisher = TestPublisher::start(&rtsp_url);
    wait_for_online_frame(&mut stream).await;
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
                | CameraStreamEvent::Frame(_) => {}
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
