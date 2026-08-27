use std::{path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use tracing_subscriber::EnvFilter;

use camwatch::{
    clips::store_segment,
    config::{CameraConfig, Config},
    ports::{CameraStream, CameraStreamEvent, CameraStreamStatus},
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
    for camera in &config.cameras {
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
                tokio::spawn(log_camera_stream(
                    camera.id.as_str().to_owned(),
                    stream,
                    Arc::clone(&status_model),
                    database.clone(),
                ));
            }
            Err(_) => {
                tracing::warn!(
                    camera_id = camera.id.as_str(),
                    "camera stream could not start"
                );
            }
        }
    }

    if !config.cameras.is_empty() {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn log_camera_stream(
    camera_id: String,
    mut stream: GstreamerCameraStream,
    status_model: Arc<CameraStatusModel>,
    database: Database,
) {
    loop {
        match stream.next_event().await {
            Ok(CameraStreamEvent::Status(status)) => {
                status_model.update(&camera_id, status);
                match status {
                    CameraStreamStatus::Online { .. } => {
                        tracing::info!(camera_id, "camera stream is online");
                    }
                    CameraStreamStatus::Offline { .. } => {
                        tracing::warn!(camera_id, "camera stream is offline");
                    }
                }
            }
            Ok(CameraStreamEvent::Frame(_)) => {}
            Ok(CameraStreamEvent::SegmentFinalized {
                path,
                started_at,
                ended_at,
            }) => {
                if let Err(error) =
                    store_segment(&database, &camera_id, path, started_at, ended_at).await
                {
                    tracing::warn!(camera_id, %error, "segment could not be stored");
                }
            }
            Err(_) => {
                tracing::warn!(camera_id, "camera stream stopped");
                return;
            }
        }
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
