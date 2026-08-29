use std::time::Duration;

use camwatch::{
    motion::{Mog2MotionDetector, MotionDetector},
    ports::{CameraStream, CameraStreamEvent},
};
use tempfile::tempdir;

use super::support::{
    BACKGROUND_LEARNING_FRAMES, MIN_MOTION_AREA, RtspSession, assemble_pets2006_mp4, camera_stream,
    pets2006_dataset, wait_for_online_frame,
};

#[tokio::test]
async fn detects_motion_from_a_live_rtsp_stream() {
    let dataset = pets2006_dataset();

    let directory = tempdir().expect("temporary directory should exist");
    let video_path = directory.path().join("pets2006.mp4");
    assemble_pets2006_mp4(&dataset, &video_path);

    let session = RtspSession::start("motion", Some(&video_path)).await;
    let mut stream = camera_stream(session.url.clone(), directory.path());
    let mut detector = Mog2MotionDetector::new().expect("MOG2 motion detector should initialize");

    wait_for_online_frame(&mut stream).await;

    tokio::time::timeout(Duration::from_secs(45), async {
        let mut learned_frames = 0usize;

        loop {
            match stream
                .next_event()
                .await
                .expect("stream should stay available")
            {
                CameraStreamEvent::Frame(frame) => {
                    let motion = detector
                        .detect(&frame)
                        .expect("motion detection should succeed");
                    if learned_frames < BACKGROUND_LEARNING_FRAMES {
                        assert_eq!(
                            motion.largest_contour_area, 0.0,
                            "RTSP frames should be ignored while the background model is learning"
                        );
                        learned_frames += 1;
                        continue;
                    }

                    if motion.largest_contour_area >= MIN_MOTION_AREA {
                        return;
                    }
                }
                CameraStreamEvent::Status(_) | CameraStreamEvent::SegmentFinalized { .. } => {}
            }
        }
    })
    .await
    .expect("MOG2 should detect motion in the PETS2006 RTSP stream after background learning");
}
