use std::sync::Arc;

use dashmap::{DashMap, mapref::entry::Entry};

type SegmentReservations = Arc<DashMap<String, usize>>;

pub(super) struct SegmentLease {
    reservations: SegmentReservations,
    paths: Vec<String>,
}

impl SegmentLease {
    pub(super) fn new(reservations: SegmentReservations) -> Self {
        Self {
            reservations,
            paths: Vec::new(),
        }
    }

    pub(super) fn reserve(&mut self, path: String) {
        if self.paths.contains(&path) {
            return;
        }

        self.reservations
            .entry(path.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        self.paths.push(path);
    }
}

impl Drop for SegmentLease {
    fn drop(&mut self) {
        for path in &self.paths {
            if let Entry::Occupied(mut entry) = self.reservations.entry(path.clone()) {
                if *entry.get() == 1 {
                    entry.remove();
                } else {
                    *entry.get_mut() -= 1;
                }
            }
        }
    }
}
