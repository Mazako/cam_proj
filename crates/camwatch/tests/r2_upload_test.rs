use std::{env, fs, time::Duration};

use aws_config::BehaviorVersion;
use aws_sdk_s3::{Client, config::Credentials};
use camwatch::{
    bucket::{BucketUploader, R2Client, UploadRequest},
    clips::Clip,
    config::Config,
};
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a real Cloudflare R2 bucket and credentials"]
async fn uploads_and_reads_a_clip_from_r2() {
    let config = Config::parse(
        r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "data/camwatch.sqlite3"
pre_event_seconds = 10
post_event_seconds = 20
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
    .expect("R2 test configuration should be valid");

    let bucket = required_env("CAMWATCH_R2_BUCKET");
    let endpoint = required_env("CAMWATCH_R2_ENDPOINT");
    let access_key_id = required_env("CAMWATCH_R2_ACCESS_KEY_ID");
    let secret_access_key = required_env("CAMWATCH_R2_SECRET_ACCESS_KEY");
    let region = env::var("CAMWATCH_R2_REGION").unwrap_or_else(|_| "auto".to_owned());
    let client = R2Client::from_app_config(&config.app).expect("R2 client should initialize");
    let verification_client = s3_client(&endpoint, &region, &access_key_id, &secret_access_key);
    let directory = tempdir().expect("temporary directory should exist");
    let clip_path = directory.path().join("clip.mp4");
    let contents = b"camwatch-r2-upload-test";
    fs::write(&clip_path, contents).expect("test clip should be written");
    let event_id = format!("camwatch-test-{}", Uuid::now_v7());

    let remote_object = client
        .upload(UploadRequest {
            event_id,
            clip: Clip::new(clip_path, Duration::from_secs(1)),
        })
        .await
        .expect("clip should upload to R2");

    assert!(remote_object.key.ends_with(".mp4"));

    let downloaded = verification_client
        .get_object()
        .bucket(&bucket)
        .key(&remote_object.key)
        .send()
        .await
        .expect("uploaded clip should be readable from R2")
        .body
        .collect()
        .await
        .expect("uploaded clip body should be readable")
        .into_bytes();

    assert_eq!(downloaded.as_ref(), contents);

    let _ = verification_client
        .delete_object()
        .bucket(bucket)
        .key(remote_object.key)
        .send()
        .await;
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set to run the R2 test"))
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
            "CAMWATCH_R2_TEST",
        ))
        .build();

    Client::from_conf(sdk_config)
}
