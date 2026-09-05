use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use camwatch::{onvif::OnvifConnection, runtime::CameraRuntime, stream::CameraStream};

pub struct RuntimeTask {
    cancel: CancellationToken,
    task: JoinHandle<()>,
    pub ptz_available: bool,
    ptz: Option<OnvifConnection>,
}

impl RuntimeTask {
    pub fn spawn<S>(runtime: CameraRuntime<S>) -> Self
    where
        S: CameraStream + 'static,
    {
        let ptz = runtime.ptz_connection();
        let ptz_available = ptz.is_some();
        let cancel = CancellationToken::new();
        let task = tokio::spawn(runtime.run(cancel.clone()));

        Self {
            cancel,
            task,
            ptz_available,
            ptz,
        }
    }

    pub fn ptz_connection(&self) -> Option<OnvifConnection> {
        self.ptz.clone()
    }

    pub fn is_running(&self) -> bool {
        !self.task.is_finished()
    }

    pub async fn stop(self) {
        self.cancel.cancel();
        let _ = self.task.await;
    }
}
