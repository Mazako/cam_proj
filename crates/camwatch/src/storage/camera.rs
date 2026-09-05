#[derive(Clone, Debug, PartialEq, sqlx::FromRow)]
pub struct Camera {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub rtsp_url: String,
    pub rtsp_codec: String,
    pub onvif_url: Option<String>,
    pub onvif_credentials: Option<String>,
    pub motion_min_area: i64,
    pub yolo_confidence: f64,
    pub clip_after_motion: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}
