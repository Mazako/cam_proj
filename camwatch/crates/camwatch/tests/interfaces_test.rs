use camwatch::{
    bucket::BucketUploader,
    motion::{MotionDetector, PersonDetector},
    stream::CameraStream,
};

fn assert_object_safe<T: ?Sized>() {}

#[test]
fn all_runtime_interfaces_are_object_safe() {
    assert_object_safe::<dyn CameraStream>();
    assert_object_safe::<dyn MotionDetector>();
    assert_object_safe::<dyn PersonDetector>();
    assert_object_safe::<dyn BucketUploader>();
}
