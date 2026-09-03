use std::sync::Arc;

use camwatch::{
    bucket::{BucketUploader, NoOpBucketUploader, R2Client},
    clips::{ClipManager, create_clip_uploader_worker, create_clip_worker, create_retainer_worker},
    config::{CameraConfig, Config},
    runtime::CameraRuntime,
    storage::{Database, NewCamera},
    stream::{CameraStatusModel, GstreamerCameraStream, SegmentRecordingConfig},
};
use dashmap::DashMap;
use std::time::Duration;

use crate::error::ServerStartupError;
use crate::runtime_task::RuntimeTask;

#[derive(Clone)]
pub struct AppState {
    pub database: Arc<Database>,
    pub clip_manager: Arc<ClipManager>,
    pub status_model: Arc<CameraStatusModel>,
    pub camera_runtimes: Arc<DashMap<String, RuntimeTask>>,
}

impl AppState {
    pub fn new(
        database: Arc<Database>,
        clip_manager: Arc<ClipManager>,
        status_model: Arc<CameraStatusModel>,
        camera_runtimes: Arc<DashMap<String, RuntimeTask>>,
    ) -> Self {
        Self {
            database,
            clip_manager,
            status_model,
            camera_runtimes,
        }
    }

    pub fn runtime_running(&self, camera_id: &str) -> bool {
        self.camera_runtimes
            .get(camera_id)
            .is_some_and(|runtime| runtime.is_running())
    }

    pub fn ptz_available(&self, camera_id: &str) -> bool {
        self.camera_runtimes
            .get(camera_id)
            .is_some_and(|runtime| runtime.ptz_available)
    }

    pub async fn stop_runtime(&self, camera_id: &str) {
        let runtime = self.camera_runtimes.remove(camera_id);
        if let Some(runtime) = runtime {
            runtime.1.stop().await;
        }
    }

    pub async fn stop_all_runtimes(&self) {
        let camera_ids = self
            .camera_runtimes
            .iter()
            .map(|entry| entry.key().clone())
            .collect::<Vec<_>>();

        for camera_id in camera_ids {
            self.stop_runtime(&camera_id).await;
        }
    }
}

pub async fn bootstrap(config: Config) -> Result<AppState, ServerStartupError> {
    let Config { app, cameras } = config;
    if !app.database_path.exists() && cameras.is_empty() {
        return Err(ServerStartupError::EmptyInitialDatabase);
    }

    let (database, was_created) = Database::open(&app.database_path)
        .await
        .map_err(ServerStartupError::Database)?;

    if !cameras.is_empty() {
        let new_cameras = cameras.iter().map(new_camera).collect::<Vec<_>>();
        database
            .upsert_cameras(&new_cameras)
            .await
            .map_err(ServerStartupError::Database)?;
    }

    let camera_count = database
        .camera_count()
        .await
        .map_err(ServerStartupError::Database)?;
    tracing::info!(
        database_created = was_created,
        camera_count,
        "camwatch initialized"
    );

    let uploader: Arc<dyn BucketUploader> = if app.r2_enabled {
        Arc::new(R2Client::from_app_config(&app).map_err(ServerStartupError::R2Configuration)?)
    } else {
        Arc::new(NoOpBucketUploader)
    };
    let upload_sender = create_clip_uploader_worker(uploader);
    let clip_sender = create_clip_worker(upload_sender);
    let database = Arc::new(database);
    let clip_manager = Arc::new(ClipManager::new(
        database.as_ref().clone(),
        clip_sender,
        app.clips_directory.clone(),
    ));
    create_retainer_worker(
        database.as_ref().clone(),
        u64::from(app.rolling_buffer_seconds),
        Arc::clone(&clip_manager),
    );

    let status_model = Arc::new(CameraStatusModel::default());
    let camera_runtimes = Arc::new(DashMap::new());
    for camera in database
        .list_cameras()
        .await
        .map_err(ServerStartupError::Database)?
    {
        let camera_id = camera.id.clone();
        let camera = CameraConfig::from_storage(camera).map_err(|source| {
            ServerStartupError::StoredCameraConfiguration {
                camera_id: camera_id.clone(),
                source,
            }
        })?;
        let camera_id = camera.id.as_str().to_owned();
        let recording = SegmentRecordingConfig::new(
            app.segment_directory.join(camera_id.as_str()),
            Duration::from_secs(u64::from(app.segment_rotation_seconds)),
        );
        match GstreamerCameraStream::from_environment(
            camera.rtsp_url_env.as_str(),
            camera.rtsp_codec,
            recording,
        ) {
            Ok(stream) => {
                let runtime = CameraRuntime::new(
                    camera,
                    &app,
                    stream,
                    Arc::clone(&status_model),
                    database.as_ref().clone(),
                    Arc::clone(&clip_manager),
                )
                .await;
                let runtime_task = RuntimeTask::spawn(runtime);
                camera_runtimes.insert(camera_id, runtime_task);
            }
            Err(_) => {
                tracing::warn!(
                    camera_id = camera.id.as_str(),
                    "camera stream could not start"
                );
            }
        }
    }

    Ok(AppState::new(
        database,
        clip_manager,
        status_model,
        camera_runtimes,
    ))
}

fn new_camera(camera: &CameraConfig) -> NewCamera {
    NewCamera {
        id: camera.id.as_str().to_owned(),
        name: camera.name.clone(),
        rtsp_url_env: camera.rtsp_url_env.as_str().to_owned(),
        rtsp_codec: camera.rtsp_codec.as_str().to_owned(),
        onvif_url: camera.onvif_url.as_ref().map(ToString::to_string),
        onvif_credentials_env: camera
            .onvif_credentials_env
            .as_ref()
            .map(|environment_variable| environment_variable.as_str().to_owned()),
        motion_min_area: i64::from(camera.motion_min_area),
        yolo_confidence: f64::from(camera.yolo_confidence),
        clip_after_motion: camera.clip_after_motion,
    }
}
