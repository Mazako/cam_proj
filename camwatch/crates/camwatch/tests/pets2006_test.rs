use std::{
    env, fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use camwatch::{
    motion::{Mog2MotionDetector, MotionDetector},
    ports::{Frame, PixelFormat},
};
use opencv::{core::MatTraitConst, imgcodecs, prelude::*};

const BACKGROUND_LEARNING_FRAMES: usize = 90;
const MIN_MOTION_AREA: f64 = 1_000.0;
const MOTION_WINDOW_END: usize = 400;

#[test]
fn detects_motion_on_pets2006_after_background_learning() {
    let Some(dataset) = pets2006_dataset() else {
        eprintln!(
            "skipping PETS2006 motion test; put the dataset in tests/resources/PETS2006 or set CAMWATCH_PETS2006"
        );
        return;
    };

    let frames = input_frames(&dataset);
    assert!(
        frames.len() >= MOTION_WINDOW_END,
        "PETS2006 should contain at least {MOTION_WINDOW_END} input frames"
    );

    let mut detector = Mog2MotionDetector::new().expect("MOG2 motion detector should initialize");
    let mut motion_after_learning = false;

    for (index, path) in frames.iter().take(MOTION_WINDOW_END).enumerate() {
        let frame = load_frame(path);
        let motion = detector
            .detect(&frame)
            .expect("motion detection should succeed");

        if index < BACKGROUND_LEARNING_FRAMES {
            assert_eq!(
                motion.largest_contour_area,
                0.0,
                "frame {} should be ignored while the background model is learning",
                index + 1
            );
            continue;
        }

        if motion.largest_contour_area >= MIN_MOTION_AREA {
            motion_after_learning = true;
        }
    }

    assert!(
        motion_after_learning,
        "MOG2 should report a contour of at least {MIN_MOTION_AREA} px after background learning"
    );
}

fn pets2006_dataset() -> Option<PathBuf> {
    if let Some(path) = env::var_os("CAMWATCH_PETS2006") {
        let dataset = PathBuf::from(path);
        assert!(
            dataset.join("input").is_dir(),
            "CAMWATCH_PETS2006 must point to a PETS2006 dataset with an input directory"
        );
        return Some(dataset);
    }

    let dataset = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/PETS2006");
    dataset.join("input").is_dir().then_some(dataset)
}

fn input_frames(dataset: &Path) -> Vec<PathBuf> {
    let mut frames = fs::read_dir(dataset.join("input"))
        .expect("PETS2006 input directory should be readable")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "jpg"))
        .collect::<Vec<_>>();
    frames.sort();
    frames
}

fn load_frame(path: &Path) -> Frame {
    let image = imgcodecs::imread(
        path.to_str().expect("PETS2006 paths should be UTF-8"),
        imgcodecs::IMREAD_COLOR,
    )
    .unwrap_or_else(|_| panic!("{} should be readable", path.display()));
    assert!(
        !image.empty(),
        "{} should decode to a non-empty BGR image",
        path.display()
    );

    Frame {
        data: image
            .data_bytes()
            .expect("decoded image should be continuous")
            .to_vec(),
        width: u32::try_from(image.cols()).expect("frame width should fit u32"),
        height: u32::try_from(image.rows()).expect("frame height should fit u32"),
        pixel_format: PixelFormat::Bgr8,
        captured_at: SystemTime::UNIX_EPOCH,
    }
}
