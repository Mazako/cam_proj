use std::{path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use tracing_subscriber::EnvFilter;

use camwatch::{
    clips::{ClipManager, create_clip_worker},
    config::{CameraConfig, Config},
    runtime::CameraRuntime,
    storage::{Database, NewCamera},
    stream::{CameraStatusModel, GstreamerCameraStream, SegmentRecordingConfig},
};

#[derive(Debug, Parser)]
#[command(name = "camwatch", version, about = "Local network camera monitoring")]
struct Cli {
    #[arg(long, env = "CAMWATCH_CONFIG", default_value = "camwatch.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() {
    init_logging();

    let cli = Cli::parse();
    let config = match Config::load(&cli.config) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Configuration error: {error}");
            std::process::exit(2);
        }
    };

    if !config.app.database_path.exists() && config.cameras.is_empty() {
        eprintln!("Configuration error: a new database requires at least one camera");
        std::process::exit(2);
    }

    let (database, was_created) = match Database::open(&config.app.database_path).await {
        Ok(database) => database,
        Err(error) => {
            eprintln!("Database error: {error}");
            std::process::exit(3);
        }
    };

    if was_created {
        let cameras = config.cameras.iter().map(new_camera).collect::<Vec<_>>();
        if let Err(error) = database.seed_cameras(&cameras).await {
            eprintln!("Database error: {error}");
            std::process::exit(3);
        }
    }

    let camera_count = match database.camera_count().await {
        Ok(camera_count) => camera_count,
        Err(error) => {
            eprintln!("Database error: {error}");
            std::process::exit(3);
        }
    };

    tracing::info!(
        database_created = was_created,
        camera_count,
        bind_address = %config.app.bind_address,
        "configuration loaded"
    );

    let status_model = Arc::new(CameraStatusModel::default());
    let has_cameras = !config.cameras.is_empty();
    let clip_sender = create_clip_worker();
    let clip_manager = Arc::new(ClipManager::new(
        database.clone(),
        clip_sender,
        config.app.clips_directory.clone(),
    ));
    for camera in config.cameras {
        let recording = SegmentRecordingConfig::new(
            config.app.segment_directory.join(camera.id.as_str()),
            Duration::from_secs(u64::from(config.app.segment_rotation_seconds)),
        );
        match GstreamerCameraStream::from_environment(
            camera.rtsp_url_env.as_str(),
            camera.rtsp_codec,
            recording,
        ) {
            Ok(stream) => {
                tokio::spawn(
                    CameraRuntime::new(
                        camera,
                        &config.app,
                        stream,
                        Arc::clone(&status_model),
                        database.clone(),
                        Arc::clone(&clip_manager),
                    )
                    .run(),
                );
            }
            Err(_) => {
                tracing::warn!(
                    camera_id = camera.id.as_str(),
                    "camera stream could not start"
                );
            }
        }
    }

    if has_cameras {
        let _ = tokio::signal::ctrl_c().await;
    }
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

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
