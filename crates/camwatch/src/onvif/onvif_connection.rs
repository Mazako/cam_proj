use std::{sync::Arc, time::Duration};

use oxvif::{OnvifError, OnvifSession};
use tokio::sync::Mutex;

use crate::config::CameraConfig;

use super::PtzDirection;

const MOVE_DURATION: Duration = Duration::from_millis(200);

#[derive(Clone)]
pub struct OnvifConnection {
    session: Arc<Mutex<OnvifSession>>,
}

impl OnvifConnection {
    pub async fn try_build(camera_config: &CameraConfig) -> Option<OnvifConnection> {
        let onvif_url = camera_config.onvif_url.as_ref()?.as_str();
        let mut builder = OnvifSession::builder(onvif_url);
        if let Some(credentials) = &camera_config.onvif_credentials {
            let mut split = credentials.splitn(2, ':');
            let username = split.next().unwrap_or_default();
            let password = split.next().unwrap_or_default();
            builder = builder.with_credentials(username, password);
        }
        let session = builder.build().await.ok()?;
        session.capabilities().ptz.url.as_ref()?;
        Some(Self {
            session: Arc::new(Mutex::new(session)),
        })
    }

    pub async fn cam_move(&self, direction: PtzDirection) -> Result<(), OnvifError> {
        let session = self.session.lock().await;
        let profiles = session.get_profiles().await?;
        let profile_token = &profiles
            .first()
            .ok_or_else(|| OnvifError::InvalidArgument("no media profile".into()))?
            .token;

        let (pan, tilt) = match direction {
            PtzDirection::Up(speed) => (0.0, speed),
            PtzDirection::Down(speed) => (0.0, -speed),
            PtzDirection::Left(speed) => (-speed, 0.0),
            PtzDirection::Right(speed) => (speed, 0.0),
        };

        session
            .ptz_continuous_move(profile_token, pan, tilt, 0.0)
            .await?;
        tokio::time::sleep(MOVE_DURATION).await;
        session.ptz_stop(profile_token).await
    }
}
