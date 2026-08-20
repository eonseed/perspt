//! Host-side `read_artifact` windows over recorder-stored bytes.
//!
//! Tool outputs above the preview bound are truncated with an
//! `artifact:<handle>` note; this module serves the stored bytes back in
//! windows small enough that the response itself always passes
//! `bounded_model_output` untouched (Gate AF, no recursion).

use anyhow::Result;

use super::contract::LoopRecorder;

/// A `read_artifact` window is smaller than the output preview bound so its
/// response (window plus header) always passes `bounded_model_output`
/// untouched — retrieval can never recurse into another artifact.
const ARTIFACT_WINDOW_MAX: usize = 7 * 1024;

/// Serve one window of a stored artifact for the `read_artifact` tool.
pub(super) fn read_artifact_window(
    recorder: Option<&dyn LoopRecorder>,
    arguments: &serde_json::Value,
) -> Result<String> {
    let handle = arguments
        .get("handle")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim()
        .trim_start_matches("artifact:");
    let offset = arguments
        .get("offset")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0);
    let limit = arguments
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(ARTIFACT_WINDOW_MAX)
        .clamp(1, ARTIFACT_WINDOW_MAX);
    let stored = recorder.and_then(|r| r.fetch_artifact(handle).transpose());
    let Some(bytes) = stored.transpose()? else {
        return Ok(format!(
            "miss: no artifact {handle} recorded in this session; artifact handles \
             appear in [full output: artifact:…] notes on truncated tool results"
        ));
    };
    let total = bytes.len();
    if offset >= total {
        return Ok(format!(
            "artifact {handle}: {total} bytes total; offset {offset} is past the end"
        ));
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut start = offset.min(text.len());
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (start + limit).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut output = format!("artifact {handle}: bytes {start}..{end} of {total}\n");
    output.push_str(&text[start..end]);
    if end < total {
        output.push_str(&format!("\n[continue with offset={end}]"));
    }
    Ok(output)
}
