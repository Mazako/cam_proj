use camwatch::ports::{
    BucketUploader, CameraStream, MotionDetector, PersonDetector, PtzController,
};

fn assert_object_safe<T: ?Sized>() {}

#[test]
fn all_ports_are_object_safe() {
    assert_object_safe::<dyn CameraStream>();
    assert_object_safe::<dyn MotionDetector>();
    assert_object_safe::<dyn PersonDetector>();
    assert_object_safe::<dyn BucketUploader>();
    assert_object_safe::<dyn PtzController>();
}
