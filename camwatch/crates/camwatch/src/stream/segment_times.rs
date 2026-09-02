use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use derive_new::new;
use gstreamer as gst;

use super::CameraStreamEvent;

#[derive(new)]
pub(super) struct SegmentTimes {
    pipeline_started_at: SystemTime,
    #[new(default)]
    opened_at: HashMap<PathBuf, SystemTime>,
}

impl SegmentTimes {
    pub(super) fn handle(&mut self, element: &gst::message::Element) -> Option<CameraStreamEvent> {
        let structure = element.structure()?;
        let location = structure.get::<String>("location").ok()?;
        let running_time = structure.get::<gst::ClockTime>("running-time").ok()?;
        let at = self
            .pipeline_started_at
            .checked_add(Duration::from_nanos(running_time.nseconds()))?;
        let path = PathBuf::from(location);

        match structure.name().as_str() {
            "splitmuxsink-fragment-opened" => {
                self.opened_at.insert(path, at);
                None
            }
            "splitmuxsink-fragment-closed" => {
                let started_at = self.opened_at.remove(&path)?;
                Some(CameraStreamEvent::SegmentFinalized {
                    path,
                    started_at,
                    ended_at: at,
                })
            }
            _ => None,
        }
    }
}
