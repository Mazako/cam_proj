use opencv::{
    core::{self, Mat},
    imgproc,
    video::{self, BackgroundSubtractorMOG2Trait},
};

use crate::ports::{Frame, Motion, MotionDetector, MotionDetectorError};

const BACKGROUND_LEARNING_FRAMES: usize = 90;
const MIN_MOTION_AREA: f64 = 1_000.0;
const MOG2_HISTORY: i32 = 500;
const MOG2_VAR_THRESHOLD: f64 = 32.0;
const MORPHOLOGY_KERNEL_SIZE: i32 = 5;

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
        let mut thresholded_mask = Mat::default();
        imgproc::threshold(
            foreground_mask,
            &mut thresholded_mask,
            200.0,
            255.0,
            imgproc::THRESH_BINARY,
        )
        .map_err(|_| MotionDetectorError::Failed)?;

        let kernel = imgproc::get_structuring_element_def(
            imgproc::MORPH_ELLIPSE,
            core::Size::new(MORPHOLOGY_KERNEL_SIZE, MORPHOLOGY_KERNEL_SIZE),
        )
        .map_err(|_| MotionDetectorError::Failed)?;
        let mut opened_mask = Mat::default();
        imgproc::morphology_ex_def(
            &thresholded_mask,
            &mut opened_mask,
            imgproc::MORPH_OPEN,
            &kernel,
        )
        .map_err(|_| MotionDetectorError::Failed)?;

        let mut dilated_mask = Mat::default();
        let empty_kernel = Mat::default();
        imgproc::dilate(
            &opened_mask,
            &mut dilated_mask,
            &empty_kernel,
            core::Point::new(-1, -1),
            2,
            core::BORDER_CONSTANT,
            core::Scalar::default(),
        )
        .map_err(|_| MotionDetectorError::Failed)?;

        let mut contours = core::Vector::<core::Vector<core::Point>>::new();
        imgproc::find_contours(
            &dilated_mask,
            &mut contours,
            imgproc::RETR_EXTERNAL,
            imgproc::CHAIN_APPROX_SIMPLE,
            core::Point::new(0, 0),
        )
        .map_err(|_| MotionDetectorError::Failed)?;

        let mut largest_area = 0.0;
        for contour in contours.iter() {
            let area =
                imgproc::contour_area(&contour, false).map_err(|_| MotionDetectorError::Failed)?;
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
