use std::f64;

use ndarray::Array4;
use opencv::{
    core::{self, _InputArrayTraitConst, Mat, MatTraitConstManual, Vec3b},
    imgproc,
};

const THRESHOLD_VALUE: f64 = 200.0;
const THRESHOLD_MAX_VALUE: f64 = 255.0;
const DILATE_ITERATIONS: i32 = 2;
const MORPHOLOGY_KERNEL_SIZE: i32 = 5;

pub(super) fn get_structuring_element() -> opencv::Result<Mat> {
    imgproc::get_structuring_element_def(
        imgproc::MORPH_ELLIPSE,
        core::Size::new(MORPHOLOGY_KERNEL_SIZE, MORPHOLOGY_KERNEL_SIZE),
    )
}

pub(super) fn threshold(src: &Mat) -> opencv::Result<Mat> {
    let mut dst = Mat::default();
    imgproc::threshold(
        src,
        &mut dst,
        THRESHOLD_VALUE,
        THRESHOLD_MAX_VALUE,
        imgproc::THRESH_BINARY,
    )?;
    Ok(dst)
}

pub(super) fn morphology_ex(src: &Mat, kernel: &Mat) -> opencv::Result<Mat> {
    let mut dst = Mat::default();
    imgproc::morphology_ex_def(src, &mut dst, imgproc::MORPH_OPEN, kernel)?;
    Ok(dst)
}

pub(super) fn dilate(src: &Mat) -> opencv::Result<Mat> {
    let mut dst = Mat::default();
    let empty_kernel = Mat::default();
    imgproc::dilate(
        src,
        &mut dst,
        &empty_kernel,
        core::Point::new(-1, -1),
        DILATE_ITERATIONS,
        core::BORDER_CONSTANT,
        core::Scalar::default(),
    )?;
    Ok(dst)
}

pub(super) fn find_contours(src: &Mat) -> opencv::Result<core::Vector<core::Vector<core::Point>>> {
    let mut contours = core::Vector::<core::Vector<core::Point>>::new();
    imgproc::find_contours(
        src,
        &mut contours,
        imgproc::RETR_EXTERNAL,
        imgproc::CHAIN_APPROX_SIMPLE,
        core::Point::new(0, 0),
    )?;
    Ok(contours)
}

pub(super) fn contour_area(contour: &core::Vector<core::Point>) -> opencv::Result<f64> {
    imgproc::contour_area(contour, false)
}

pub(super) fn letterbox(
    src: &impl core::ToInputArray,
    width: i32,
    height: i32,
) -> opencv::Result<Mat> {
    let inp = src.input_array()?;
    let src_w = inp.size_def()?.width;
    let src_h = inp.size_def()?.height;

    let scale = f64::min(width as f64 / src_w as f64, height as f64 / src_h as f64);

    let resized_size =
        core::Size::new((src_w as f64 * scale) as i32, (src_h as f64 * scale) as i32);
    let mut resized_mat = Mat::default();
    imgproc::resize(
        src,
        &mut resized_mat,
        resized_size,
        0.0,
        0.0,
        imgproc::INTER_LINEAR,
    )?;

    let pad_w = width - resized_size.width;
    let pad_h = height - resized_size.height;

    let left = pad_w / 2;
    let right = pad_w - left;
    let top = pad_h / 2;
    let bottom = pad_h - top;

    let mut dst = Mat::default();
    core::copy_make_border(
        &resized_mat,
        &mut dst,
        top,
        bottom,
        left,
        right,
        core::BORDER_CONSTANT,
        core::Scalar::new(114.0, 114.0, 114.0, 0.0),
    )?;

    Ok(dst)
}

pub(super) fn mat_to_yolo_tensor(mat: &impl core::ToInputArray) -> opencv::Result<Array4<f32>> {
    let resized = letterbox(mat, 640, 640)?;
    let mut rgb = Mat::default();
    imgproc::cvt_color(
        &resized,
        &mut rgb,
        imgproc::COLOR_BGR2RGB,
        0,
        core::AlgorithmHint::ALGO_HINT_DEFAULT
    )?;
    let pixels = rgb.data_typed::<Vec3b>()?;
    let arr = Array4::from_shape_fn((1, 3, 640, 640), |(_, c, y, x)| {
        f32::from(pixels[y * 640 + x][c]) / 255.0
    });
    Ok(arr)
}

#[cfg(test)]
mod tests {
    use opencv::{
        core::{Mat, Scalar, Vec3b},
        prelude::*,
    };

    use super::mat_to_yolo_tensor;

    #[test]
    fn converts_bgr_mat_to_normalized_rgb_nchw_tensor() -> opencv::Result<()> {
        let mut mat =
            Mat::new_rows_cols_with_default(1, 1, Vec3b::opencv_type(), Scalar::default())?;
        *mat.at_2d_mut::<Vec3b>(0, 0)? = Vec3b::from([10, 20, 30]);

        let tensor = mat_to_yolo_tensor(&mat)?;

        assert_eq!(tensor.shape(), [1, 3, 640, 640]);
        assert_eq!(tensor[[0, 0, 0, 0]], 30.0 / 255.0);
        assert_eq!(tensor[[0, 1, 0, 0]], 20.0 / 255.0);
        assert_eq!(tensor[[0, 2, 0, 0]], 10.0 / 255.0);

        Ok(())
    }
}
