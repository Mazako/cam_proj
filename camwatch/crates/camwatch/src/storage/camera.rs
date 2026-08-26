#[derive(Clone, Debug, PartialEq)]
pub struct NewCamera {
    pub id: String,
    pub name: String,
    pub rtsp_url_env: String,
    pub onvif_url: Option<String>,
    pub onvif_credentials_env: Option<String>,
    pub motion_min_area: i64,
    pub yolo_confidence: f64,
}

#[derive(Clone, Debug, PartialEq, sqlx::FromRow)]
pub struct Camera {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub rtsp_url_env: String,
    pub onvif_url: Option<String>,
    pub onvif_credentials_env: Option<String>,
    pub motion_min_area: i64,
    pub yolo_confidence: f64,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}
