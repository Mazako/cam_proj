use std::{collections::HashSet, env, fs, sync::Arc, time::Duration};

use aws_config::BehaviorVersion;
use aws_sdk_s3::{Client, config::Credentials};
use camwatch::{
    bucket::{BucketUploader, R2Client},
    clips::{
        ClipManager, create_clip, create_clip_uploader_worker, create_clip_worker, store_segment,
    },
    config::{AppConfig, CameraConfig, Config},
    runtime::CameraRuntime,
    stream::CameraStatusModel,
};
use tempfile::tempdir;

use super::support::{
    RtspSession, assemble_pets2006_mp4, camera_stream, database_with_camera, is_playable_mp4,
    pets2006_dataset, wait_for_finalized_segment, wait_for_online_frame,
};

#[tokio::test]
async fn assembles_a_clip_from_persisted_rtsp_segments() {
    let session = RtspSession::start("clips", None).await;
    let directory = tempdir().expect("temporary directory should exist");
    let mut stream = camera_stream(session.url.clone(), &directory.path().join("segments"));
    let database = database_with_camera(directory.path()).await;

    wait_for_online_frame(&mut stream).await;
    let first = wait_for_finalized_segment(&mut stream).await;
    let second = wait_for_finalized_segment(&mut stream).await;

    for (path, started_at, ended_at) in [&first, &second] {
        store_segment(
            &database,
            "front-door",
            path.clone(),
            *started_at,
            *ended_at,
        )
        .await
        .expect("finalized segment should be stored");
    }

    let clip = create_clip(
        &database,
        "front-door",
        first.1,
        second.2,
        directory.path().join("clips/event-1.mp4"),
    )
    .await
    .expect("clip should be assembled");

    assert!(clip.path.is_file());
    assert!(clip.duration > Duration::ZERO);
    assert!(is_playable_mp4(&clip.path));
}

#[tokio::test]
#[ignore = "requires Docker, FFmpeg, GStreamer, and a real Cloudflare R2 bucket"]
async fn records_assembles_and_uploads_a_clip_to_r2() {
    let r2 = r2_environment();
    let verification_client = s3_client(
        &r2.endpoint,
        &r2.region,
        &r2.access_key_id,
        &r2.secret_access_key,
    );
    let objects_before = list_objects(&verification_client, &r2.bucket, &r2.prefix).await;
    let directory = tempdir().expect("temporary directory should exist");
    let video_path = directory.path().join("pets2006.mp4");
    assemble_pets2006_mp4(&pets2006_dataset(), &video_path);
    let database = database_with_camera(directory.path()).await;
    let session = RtspSession::start("full-r2", Some(&video_path)).await;
    let app_config = app_config(directory.path());
    let stream = camera_stream(
        session.url.clone(),
        &app_config.segment_directory.join("front-door"),
    );
    let uploader: Arc<dyn BucketUploader> =
        Arc::new(R2Client::from_app_config(&r2.config.app).expect("R2 client should initialize"));
    let upload_sender = create_clip_uploader_worker(uploader);
    let clip_sender = create_clip_worker(upload_sender);
    let clip_manager = Arc::new(ClipManager::new(
        database.clone(),
        clip_sender,
        app_config.clips_directory.clone(),
    ));
    let runtime = CameraRuntime::new(
        camera_config(),
        &app_config,
        stream,
        Arc::new(CameraStatusModel::default()),
        database,
        clip_manager,
    )
    .await;
    let runtime_task = tokio::spawn(runtime.run());

    let uploaded_key = tokio::time::timeout(Duration::from_secs(90), async {
        loop {
            let objects = list_objects(&verification_client, &r2.bucket, &r2.prefix).await;
            if let Some(key) = objects.difference(&objects_before).next() {
                break key.clone();
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
    .await
    .expect("the full pipeline should upload a clip before timeout");
    runtime_task.abort();

    let downloaded = verification_client
        .get_object()
        .bucket(&r2.bucket)
        .key(&uploaded_key)
        .send()
        .await
        .expect("the uploaded clip should be readable from R2")
        .body
        .collect()
        .await
        .expect("the uploaded clip body should be readable")
        .into_bytes();
    let downloaded_path = directory.path().join("downloaded.mp4");
    fs::write(&downloaded_path, downloaded).expect("downloaded clip should be written");

    assert!(downloaded_path.metadata().expect("clip should exist").len() > 0);
    assert!(is_playable_mp4(&downloaded_path));

    let _ = verification_client
        .delete_object()
        .bucket(r2.bucket)
        .key(uploaded_key)
        .send()
        .await;
}

struct R2Environment {
    endpoint: String,
    access_key_id: String,
    secret_access_key: String,
    bucket: String,
    prefix: String,
    region: String,
    config: Config,
}

fn r2_environment() -> R2Environment {
    let config = Config::parse(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "data/camwatch.sqlite3"
pre_event_seconds = 1
post_event_seconds = 1
rolling_buffer_seconds = 30
r2_enabled = true
r2_endpoint_env = "CAMWATCH_R2_ENDPOINT"
r2_access_key_id_env = "CAMWATCH_R2_ACCESS_KEY_ID"
r2_secret_access_key_env = "CAMWATCH_R2_SECRET_ACCESS_KEY"
r2_bucket_env = "CAMWATCH_R2_BUCKET"
r2_prefix_env = "CAMWATCH_R2_PREFIX"
r2_region_env = "CAMWATCH_R2_REGION"
"#,
    )
    .expect("R2 configuration should be valid");

    R2Environment {
        endpoint: required_env("CAMWATCH_R2_ENDPOINT"),
        access_key_id: required_env("CAMWATCH_R2_ACCESS_KEY_ID"),
        secret_access_key: required_env("CAMWATCH_R2_SECRET_ACCESS_KEY"),
        bucket: required_env("CAMWATCH_R2_BUCKET"),
        prefix: env::var("CAMWATCH_R2_PREFIX").unwrap_or_default(),
        region: env::var("CAMWATCH_R2_REGION").unwrap_or_else(|_| "auto".to_owned()),
        config,
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set to run the R2 test"))
}

async fn list_objects(client: &Client, bucket: &str, prefix: &str) -> HashSet<String> {
    client
        .list_objects_v2()
        .bucket(bucket)
        .prefix(prefix)
        .send()
        .await
        .expect("R2 objects should be listable")
        .contents()
        .iter()
        .filter_map(|object| object.key().map(ToOwned::to_owned))
        .collect()
}

fn s3_client(endpoint: &str, region: &str, access_key_id: &str, secret_access_key: &str) -> Client {
    let sdk_config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .endpoint_url(endpoint)
        .region(aws_sdk_s3::config::Region::new(region.to_owned()))
        .credentials_provider(Credentials::new(
            access_key_id,
            secret_access_key,
            None,
            None,
            "CAMWATCH_R2_FULL_TEST",
        ))
        .build();

    Client::from_conf(sdk_config)
}

fn camera_config() -> CameraConfig {
    Config::parse(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "data/camwatch.sqlite3"
pre_event_seconds = 1
post_event_seconds = 1
rolling_buffer_seconds = 30
segment_rotation_seconds = 1

[[cameras]]
id = "front-door"
name = "Front door"
rtsp_url_env = "CAMWATCH_FRONT_DOOR_RTSP_URL"
motion_min_area = 1000
yolo_confidence = 0.5
clip_after_motion = true
"#,
    )
    .expect("camera configuration should be valid")
    .cameras
    .into_iter()
    .next()
    .expect("camera configuration should contain a camera")
}

fn app_config(directory: &std::path::Path) -> AppConfig {
    let input = format!(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "{}"
pre_event_seconds = 1
post_event_seconds = 1
rolling_buffer_seconds = 30
segment_rotation_seconds = 1
segment_directory = "{}"
clips_directory = "{}"
"#,
        directory.join("camwatch.sqlite3").display(),
        directory.join("segments").display(),
        directory.join("clips").display(),
    );
    Config::parse(&input)
        .expect("application configuration should be valid")
        .app
}
