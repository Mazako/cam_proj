use opencv::{
    core::{self, Mat},
    video::{self, BackgroundSubtractorMOG2Trait},
};

use crate::ports::{Frame, Motion, MotionDetector, MotionDetectorError};

use super::cv2_helper;

const BACKGROUND_LEARNING_FRAMES: usize = 90;
const MIN_MOTION_AREA: f64 = 1_000.0;
const MOG2_HISTORY: i32 = 500;
const MOG2_VAR_THRESHOLD: f64 = 32.0;

pub struct Mog2MotionDetector {
    mog2: core::Ptr<video::BackgroundSubtractorMOG2>,
    learning_frames_remaining: usize,
}

impl Mog2MotionDetector {
    pub fn new() -> Result<Self, MotionDetectorError> {
        Ok(Self {
            mog2: create_mog2()?,
            learning_frames_remaining: BACKGROUND_LEARNING_FRAMES,
        })
    }

    fn foreground_mask(&mut self, frame: &Frame) -> Result<Mat, MotionDetectorError> {
        let image = frame.as_mat().map_err(|_| MotionDetectorError::Failed)?;
        let mut foreground_mask = Mat::default();
        BackgroundSubtractorMOG2Trait::apply_def(&mut self.mog2, &image, &mut foreground_mask)
            .map_err(|_| MotionDetectorError::Failed)?;
        Ok(foreground_mask)
    }

    fn largest_motion_area(&self, foreground_mask: &Mat) -> Result<f64, MotionDetectorError> {
        let thresholded_mask =
            cv2_helper::threshold(foreground_mask).map_err(|_| MotionDetectorError::Failed)?;

        let kernel =
            cv2_helper::get_structuring_element().map_err(|_| MotionDetectorError::Failed)?;
        let opened_mask = cv2_helper::morphology_ex(&thresholded_mask, &kernel)
            .map_err(|_| MotionDetectorError::Failed)?;

        let dilated_mask =
            cv2_helper::dilate(&opened_mask).map_err(|_| MotionDetectorError::Failed)?;

        let contours =
            cv2_helper::find_contours(&dilated_mask).map_err(|_| MotionDetectorError::Failed)?;

        let mut largest_area = 0.0;
        for contour in contours.iter() {
            let area =
                cv2_helper::contour_area(&contour).map_err(|_| MotionDetectorError::Failed)?;
            if area >= MIN_MOTION_AREA && area > largest_area {
                largest_area = area;
            }
        }

        Ok(largest_area)
    }
}

impl MotionDetector for Mog2MotionDetector {
    fn detect(&mut self, frame: &Frame) -> Result<Motion, MotionDetectorError> {
        let foreground_mask = self.foreground_mask(frame)?;

        if self.learning_frames_remaining > 0 {
            self.learning_frames_remaining -= 1;
            return Ok(Motion::new(0.0));
        }

        Ok(Motion::new(self.largest_motion_area(&foreground_mask)?))
    }

    fn reset(&mut self) -> Result<(), MotionDetectorError> {
        self.mog2 = create_mog2()?;
        self.learning_frames_remaining = BACKGROUND_LEARNING_FRAMES;
        Ok(())
    }
}

fn create_mog2() -> Result<core::Ptr<video::BackgroundSubtractorMOG2>, MotionDetectorError> {
    video::create_background_subtractor_mog2(MOG2_HISTORY, MOG2_VAR_THRESHOLD, true)
        .map_err(|_| MotionDetectorError::Failed)
}
