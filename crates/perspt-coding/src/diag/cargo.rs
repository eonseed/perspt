//! `cargo --message-format=json` diagnostics (PSP-10 system 26).
//!
//! Preserves codes, primary spans, children, and applicable suggestions
//! from the compiler's native JSON stream. Non-JSON lines are ignored, so
//! mixed streams (human progress on stderr, JSON on stdout) parse cleanly.

use perspt_sdk::ResidualClass;

use super::types::StructuredDiagnostic;
use crate::lang::classify_rust_code;

/// Parser identity; enters the sensor fingerprint.
pub const PARSER_ID: &str = "cargo-json-v1";

/// Whether a captured output looks like a cargo JSON stream.
pub fn looks_like_stream(raw: &str) -> bool {
    raw.lines()
        .any(|line| line.starts_with('{') && line.contains("\"reason\""))
}

/// Parse every `compiler-message` with level `error` from the stream.
pub fn parse(raw: &str) -> Vec<StructuredDiagnostic> {
    let mut diagnostics = Vec::new();
    for line in raw.lines() {
        if !line.starts_with('{') {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") {
            continue;
        }
        let Some(message) = value.get("message") else {
            continue;
        };
        if message.get("level").and_then(|l| l.as_str()) != Some("error") {
            continue;
        }
        diagnostics.push(from_message(message));
    }
    diagnostics
}

fn from_message(message: &serde_json::Value) -> StructuredDiagnostic {
    let code = message
        .get("code")
        .and_then(|code| code.get("code"))
        .and_then(|code| code.as_str())
        .map(str::to_string);
    let class = code
        .as_deref()
        .map(classify_rust_code)
        .unwrap_or(ResidualClass::Build);
    let span = message
        .get("spans")
        .and_then(|spans| spans.as_array())
        .and_then(|spans| {
            spans
                .iter()
                .find(|span| span.get("is_primary").and_then(|p| p.as_bool()) == Some(true))
                .or_else(|| spans.first())
        });
    let suggestion = span
        .and_then(|span| span.get("suggested_replacement"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or_else(|| {
            message
                .get("children")
                .and_then(|children| children.as_array())
                .and_then(|children| {
                    children.iter().find_map(|child| {
                        child
                            .get("spans")?
                            .as_array()?
                            .iter()
                            .find_map(|span| span.get("suggested_replacement")?.as_str())
                            .map(str::to_string)
                    })
                })
        });
    StructuredDiagnostic {
        class,
        code,
        message: message
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or_default()
            .to_string(),
        path: span
            .and_then(|span| span.get("file_name"))
            .and_then(|f| f.as_str())
            .map(str::to_string),
        line: span
            .and_then(|span| span.get("line_start"))
            .and_then(|l| l.as_u64())
            .and_then(|l| u32::try_from(l).ok()),
        column: span
            .and_then(|span| span.get("column_start"))
            .and_then(|c| c.as_u64())
            .and_then(|c| u32::try_from(c).ok()),
        primary: true,
        suggestion,
    }
}
