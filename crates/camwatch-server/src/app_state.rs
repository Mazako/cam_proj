use std::sync::Arc;

use camwatch::{
    bucket::{BucketUploader, NoOpBucketUploader, R2Client},
    clips::{ClipManager, create_clip_uploader_worker, create_clip_worker, create_retainer_worker},
    config::{AppConfig, CameraConfig, Config, SecretManager},
    runtime::CameraRuntime,
    storage::{Camera, Database, NewCamera, StorageError},
    stream::{CameraStatusModel, GstreamerCameraStream, SegmentRecordingConfig},
};
use dashmap::DashMap;
use std::time::Duration;

use crate::auth::AuthService;
use crate::camera_dto::{CameraDetailsDto, CameraSummaryDto};
use crate::error::{RuntimeReloadError, ServerStartupError};
use crate::runtime_task::RuntimeTask;

#[derive(Clone)]
pub struct AppState {
    pub auth: Arc<AuthService>,
    pub database: Arc<Database>,
    pub clip_manager: Arc<ClipManager>,
    pub status_model: Arc<CameraStatusModel>,
    pub camera_runtimes: Arc<DashMap<String, RuntimeTask>>,
    pub runtime_config: Arc<AppConfig>,
    pub secret_manager: Arc<SecretManager>,
}

impl AppState {
    pub fn new(
        auth: Arc<AuthService>,
        database: Arc<Database>,
        clip_manager: Arc<ClipManager>,
        status_model: Arc<CameraStatusModel>,
        camera_runtimes: Arc<DashMap<String, RuntimeTask>>,
        runtime_config: Arc<AppConfig>,
        secret_manager: Arc<SecretManager>,
    ) -> Self {
        Self {
            auth,
            database,
            clip_manager,
            status_model,
            camera_runtimes,
            runtime_config,
            secret_manager,
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
            .is_some_and(|runtime| runtime.is_running() && runtime.ptz_available)
    }

    pub async fn camera_summaries(&self) -> Result<Vec<CameraSummaryDto>, StorageError> {
        Ok(self
            .database
            .list_cameras()
            .await?
            .iter()
            .map(|camera| self.camera_summary(camera))
            .collect())
    }

    pub async fn camera_details(
        &self,
        camera_id: &str,
    ) -> Result<Option<CameraDetailsDto>, StorageError> {
        let Some(camera) = self.database.get_camera(camera_id).await? else {
            return Ok(None);
        };
        if !camera.enabled || camera.deleted_at.is_some() {
            return Ok(None);
        }

        let summary = self.camera_summary(&camera);
        Ok(Some(CameraDetailsDto {
            summary,
            enabled: camera.enabled,
            rtsp_url: camera.rtsp_url,
            onvif_url: camera.onvif_url,
            onvif_credentials: camera.onvif_credentials,
            motion_min_area: camera.motion_min_area,
            yolo_confidence: camera.yolo_confidence,
            clip_after_motion: camera.clip_after_motion,
        }))
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

    pub async fn replace_camera_runtime(&self, camera: Camera) -> Result<(), RuntimeReloadError> {
        let camera_id = camera.id.clone();
        let camera = CameraConfig::from_storage(camera, &self.secret_manager)
            .map_err(RuntimeReloadError::InvalidConfiguration)?;
        let recording = SegmentRecordingConfig::new(
            self.runtime_config
                .segment_directory
                .join(camera_id.as_str()),
            Duration::from_secs(u64::from(self.runtime_config.segment_rotation_seconds)),
        );
        let stream = GstreamerCameraStream::new(camera.rtsp_url.clone(), recording)
            .map_err(|_| RuntimeReloadError::StreamUnavailable)?;
        let runtime = CameraRuntime::new(
            camera,
            &self.runtime_config,
            stream,
            Arc::clone(&self.status_model),
            self.database.as_ref().clone(),
            Arc::clone(&self.clip_manager),
        )
        .await;

        self.stop_runtime(&camera_id).await;
        self.camera_runtimes
            .insert(camera_id, RuntimeTask::spawn(runtime));
        Ok(())
    }

    fn camera_summary(&self, camera: &Camera) -> CameraSummaryDto {
        CameraSummaryDto {
            id: camera.id.clone(),
            name: camera.name.clone(),
            runtime_running: self.runtime_running(&camera.id),
            stream_status: self.status_model.get(&camera.id),
            ptz_available: self.ptz_available(&camera.id),
        }
    }
}

pub async fn bootstrap(config: Config) -> Result<AppState, ServerStartupError> {
    let secret_manager =
        Arc::new(SecretManager::from_environment().map_err(ServerStartupError::Secrets)?);
    bootstrap_with_secret_manager(config, secret_manager).await
}

pub async fn bootstrap_with_secret_manager(
    config: Config,
    secret_manager: Arc<SecretManager>,
) -> Result<AppState, ServerStartupError> {
    let config = config
        .decrypt_secrets(&secret_manager)
        .map_err(ServerStartupError::Configuration)?;
    let Config { app, cameras } = config;
    let auth = Arc::new(
        AuthService::from_environment().map_err(ServerStartupError::AuthenticationConfiguration)?,
    );
    if !app.database_path.exists() && cameras.is_empty() {
        return Err(ServerStartupError::EmptyInitialDatabase);
    }

    let (database, was_created) = Database::open(&app.database_path)
        .await
        .map_err(ServerStartupError::Database)?;

    if !cameras.is_empty() {
        let new_cameras = cameras
            .iter()
            .map(|camera| new_camera(camera, &secret_manager))
            .collect::<Result<Vec<_>, _>>()
            .map_err(ServerStartupError::Secrets)?;
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
        let camera = CameraConfig::from_storage(camera, &secret_manager).map_err(|source| {
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
        match GstreamerCameraStream::new(camera.rtsp_url.clone(), recording) {
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
        auth,
        database,
        clip_manager,
        status_model,
        camera_runtimes,
        Arc::new(app),
        secret_manager,
    ))
}

fn new_camera(
    camera: &CameraConfig,
    secret_manager: &SecretManager,
) -> Result<NewCamera, camwatch::config::SecretError> {
    camera.to_storage(secret_manager)
}
