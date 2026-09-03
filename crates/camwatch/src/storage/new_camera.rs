#[derive(Clone, Debug, PartialEq)]
pub struct NewCamera {
    pub id: String,
    pub name: String,
    pub rtsp_url_env: String,
    pub rtsp_codec: String,
    pub onvif_url: Option<String>,
    pub onvif_credentials_env: Option<String>,
    pub motion_min_area: i64,
    pub yolo_confidence: f64,
    pub clip_after_motion: bool,
}
