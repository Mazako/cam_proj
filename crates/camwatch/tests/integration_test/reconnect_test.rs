use tempfile::tempdir;

use super::support::{
    RtspSession, TestPublisher, camera_stream, wait_for_offline, wait_for_online_frame,
};

#[tokio::test]
async fn returns_online_after_the_rtsp_publisher_restarts() {
    let mut session = RtspSession::start("reconnect", None).await;
    let directory = tempdir().expect("temporary directory should exist");
    let mut stream = camera_stream(session.url.clone(), directory.path());

    wait_for_online_frame(&mut stream).await;

    session.publisher.stop();
    wait_for_offline(&mut stream).await;

    session.publisher = TestPublisher::start(&session.url, None);
    wait_for_online_frame(&mut stream).await;
}
