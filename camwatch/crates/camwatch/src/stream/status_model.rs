use std::{collections::HashMap, sync::RwLock};

use crate::ports::CameraStreamStatus;

#[derive(Default)]
pub struct CameraStatusModel {
    statuses: RwLock<HashMap<String, CameraStreamStatus>>,
}

impl CameraStatusModel {
    pub fn update(&self, camera_id: &str, status: CameraStreamStatus) {
        self.statuses
            .write()
            .expect("camera status model lock is not poisoned")
            .insert(camera_id.to_owned(), status);
    }

    pub fn get(&self, camera_id: &str) -> Option<CameraStreamStatus> {
        self.statuses
            .read()
            .expect("camera status model lock is not poisoned")
            .get(camera_id)
            .copied()
    }
}
