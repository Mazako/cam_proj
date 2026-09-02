use derive_new::new;

#[derive(Clone, Copy, Debug, PartialEq, new)]
pub struct Motion {
    pub largest_contour_area: f64,
}
