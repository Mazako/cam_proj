use super::RtspCodec;

pub fn pipeline_description(rtsp_url: &str, codec: RtspCodec) -> String {
    let rtsp_url = rtsp_url.replace('\\', "\\\\").replace('"', "\\\"");
    let codec_elements = match codec {
        RtspCodec::H264 => "rtph264depay ! h264parse ! avdec_h264",
        RtspCodec::H265 => "rtph265depay ! h265parse ! avdec_h265",
    };

    format!(
        "rtspsrc location=\"{rtsp_url}\" protocols=tcp latency=200 tcp-timeout=5000000 ! {codec_elements} ! videoconvert ! video/x-raw,format=BGR ! appsink name=analysis_sink sync=false max-buffers=1 drop=true"
    )
}
