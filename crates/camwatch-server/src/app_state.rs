use std::sync::Arc;

use camwatch::{
    bucket::{BucketUploader, NoOpBucketUploader, R2Client},
    clips::{ClipManager, create_clip_uploader_worker, create_clip_worker, create_retainer_worker},
    config::{CameraConfig, Config},
    runtime::CameraRuntime,
    storage::{Database, NewCamera},
    stream::{CameraStatusModel, GstreamerCameraStream, SegmentRecordingConfig},
};
use std::time::Duration;

use crate::error::ServerStartupError;

#[derive(Clone)]
pub struct AppState {
    pub database: Arc<Database>,
    pub clip_manager: Arc<ClipManager>,
    pub status_model: Arc<CameraStatusModel>,
}

impl AppState {
    pub fn new(
        database: Arc<Database>,
        clip_manager: Arc<ClipManager>,
        status_model: Arc<CameraStatusModel>,
    ) -> Self {
        Self {
            database,
            clip_manager,
            status_model,
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

    if was_created {
        let new_cameras = cameras.iter().map(new_camera).collect::<Vec<_>>();
        database
            .seed_cameras(&new_cameras)
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
    for camera in cameras {
        let recording = SegmentRecordingConfig::new(
            app.segment_directory.join(camera.id.as_str()),
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
                tokio::spawn(runtime.run());
            }
            Err(_) => {
                tracing::warn!(
                    camera_id = camera.id.as_str(),
                    "camera stream could not start"
                );
            }
        }
    }

    Ok(AppState::new(database, clip_manager, status_model))
}

fn new_camera(camera: &CameraConfig) -> NewCamera {
    NewCamera {
        id: camera.id.as_str().to_owned(),
        name: camera.name.clone(),
        rtsp_url_env: camera.rtsp_url_env.as_str().to_owned(),
        onvif_url: camera.onvif_url.as_ref().map(ToString::to_string),
        onvif_credentials_env: camera
            .onvif_credentials_env
            .as_ref()
            .map(|environment_variable| environment_variable.as_str().to_owned()),
        motion_min_area: i64::from(camera.motion_min_area),
        yolo_confidence: f64::from(camera.yolo_confidence),
    }
}
