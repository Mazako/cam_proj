use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use gstreamer::{self as gst, prelude::*};
use gstreamer_pbutils as gst_pbutils;
use url::Url;

use crate::stream::escape_pipeline_value;

use super::{Clip, ClipStoreError, Segment};

pub(crate) async fn create_clip_from_segments(
    segments: Vec<std::sync::Arc<Segment>>,
    output_path: PathBuf,
) -> Result<Clip, ClipStoreError> {
    if segments.is_empty() {
        return Err(ClipStoreError::NoSegments);
    }

    tokio::task::spawn_blocking(move || assemble_clip(segments, output_path))
        .await
        .map_err(|_| ClipStoreError::AssemblyTask)?
}

fn assemble_clip(
    segments: Vec<std::sync::Arc<Segment>>,
    output_path: PathBuf,
) -> Result<Clip, ClipStoreError> {
    gst::init().map_err(|_| ClipStoreError::GstreamerInitialization)?;

    let output_directory = output_path
        .parent()
        .filter(|directory| !directory.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(output_directory).map_err(ClipStoreError::CreateDirectory)?;

    let staged_segments = tempfile::Builder::new()
        .prefix("camwatch-clip-")
        .tempdir_in(output_directory)
        .map_err(ClipStoreError::TemporaryDirectory)?;
    for (index, segment) in segments.iter().enumerate() {
        fs::copy(
            &segment.path,
            staged_segments
                .path()
                .join(format!("segment-{index:010}.mp4")),
        )
        .map_err(ClipStoreError::StageSegment)?;
    }

    let input_location = staged_segments.path().join("segment-*.mp4");
    let description = format!(
        concat!(
            "splitmuxsrc ",
            "location=\"{}\" ",
            "! parsebin ",
            "! mp4mux ",
            "faststart=true ",
            "! filesink ",
            "location=\"{}\""
        ),
        escape_pipeline_value(input_location.to_string_lossy().as_ref()),
        escape_pipeline_value(output_path.to_string_lossy().as_ref()),
    );
    let pipeline = gst::parse::launch(&description)
        .map_err(|_| ClipStoreError::PipelineBuild)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| ClipStoreError::PipelineBuild)?;
    let bus = pipeline.bus().ok_or(ClipStoreError::PipelineBuild)?;

    pipeline
        .set_state(gst::State::Playing)
        .map_err(|_| ClipStoreError::PipelineStart)?;

    loop {
        let Some(message) = bus.timed_pop(gst::ClockTime::NONE) else {
            let _ = pipeline.set_state(gst::State::Null);
            return Err(ClipStoreError::PipelineExecution);
        };

        match message.view() {
            gst::MessageView::Eos(..) => break,
            gst::MessageView::Error(..) => {
                let _ = pipeline.set_state(gst::State::Null);
                return Err(ClipStoreError::PipelineExecution);
            }
            _ => {}
        }
    }

    let _ = pipeline.set_state(gst::State::Null);
    let duration = clip_duration(&output_path)?;

    Ok(Clip::new(output_path, duration))
}

fn clip_duration(path: &Path) -> Result<Duration, ClipStoreError> {
    let path = fs::canonicalize(path).map_err(|_| ClipStoreError::ClipMetadata)?;
    let uri = Url::from_file_path(path).map_err(|_| ClipStoreError::InvalidPath)?;
    let discoverer = gst_pbutils::Discoverer::new(gst::ClockTime::from_seconds(5))
        .map_err(|_| ClipStoreError::ClipMetadata)?;
    let info = discoverer
        .discover_uri(uri.as_str())
        .map_err(|_| ClipStoreError::ClipMetadata)?;
    let duration = info.duration().ok_or(ClipStoreError::ClipMetadata)?;

    Ok(Duration::from_nanos(duration.nseconds()))
}
