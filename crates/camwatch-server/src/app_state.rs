use std::sync::Arc;

use camwatch::CamwatchHandle;

#[derive(Clone)]
pub struct AppState {
    pub camwatch: Arc<CamwatchHandle>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            camwatch: Arc::new(CamwatchHandle),
        }
    }
}
