use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use camwatch::{runtime::CameraRuntime, stream::CameraStream};

pub struct RuntimeTask {
    cancel: CancellationToken,
    task: JoinHandle<()>,
    pub ptz_available: bool,
}

impl RuntimeTask {
    pub fn spawn<S>(runtime: CameraRuntime<S>) -> Self
    where
        S: CameraStream + 'static,
    {
        let ptz_available = runtime.has_ptz();
        let cancel = CancellationToken::new();
        let task = tokio::spawn(runtime.run(cancel.clone()));

        Self {
            cancel,
            task,
            ptz_available,
        }
    }

    pub fn is_running(&self) -> bool {
        !self.task.is_finished()
    }

    pub async fn stop(self) {
        self.cancel.cancel();
        let _ = self.task.await;
    }
}
