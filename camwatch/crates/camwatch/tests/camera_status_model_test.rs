use std::time::SystemTime;

use camwatch::stream::{CameraStatusModel, CameraStreamStatus};

#[test]
fn stores_the_latest_status_for_each_camera() {
    let model = CameraStatusModel::default();
    let online_at = SystemTime::UNIX_EPOCH;
    let offline_at = online_at + std::time::Duration::from_secs(1);

    model.update(
        "front-door",
        CameraStreamStatus::Online { since: online_at },
    );
    model.update(
        "front-door",
        CameraStreamStatus::Offline { since: offline_at },
    );

    assert_eq!(
        model.get("front-door"),
        Some(CameraStreamStatus::Offline { since: offline_at })
    );
    assert_eq!(model.get("back-yard"), None);
}
