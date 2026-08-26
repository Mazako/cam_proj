use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RtspCodec {
    #[default]
    H264,
    H265,
}
