//! `ty` diagnostics (PSP-10 system 26): the primary Python sensor.
//!
//! The LSP observation is primary; when it is empty the runtime falls back
//! to `ty check --output-format concise`, whose versioned text format is
//! `path:line:col: error[rule] message` (sampled against ty 0.0.x). The
//! PSP claims no JSON format for `ty`.

use perspt_sdk::ResidualClass;

use super::types::StructuredDiagnostic;

/// Text-parser identity; enters the sensor fingerprint.
pub const PARSER_ID: &str = "ty-check-text-v1";

fn classify_rule(rule: &str) -> ResidualClass {
    match rule {
        "unresolved-import" | "unresolved-module" => ResidualClass::ImportGraph,
        "unresolved-reference" | "unresolved-attribute" | "possibly-unresolved-reference" => {
            ResidualClass::SymbolMismatch
        }
        "invalid-assignment"
        | "invalid-argument-type"
        | "invalid-return-type"
        | "unsupported-operator"
        | "invalid-type-form" => ResidualClass::Type,
        _ => ResidualClass::Type,
    }
}

/// Parse `ty check --output-format concise` lines.
pub fn parse_check_text(raw: &str) -> Vec<StructuredDiagnostic> {
    raw.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<StructuredDiagnostic> {
    // `bad.py:1:8: error[unresolved-import] Cannot resolve ...`
    let (location, rest) = line.split_once(": error[")?;
    let (rule, message) = rest.split_once("] ")?;
    let mut parts = location.rsplitn(3, ':');
    let column: u32 = parts.next()?.parse().ok()?;
    let line_no: u32 = parts.next()?.parse().ok()?;
    let path = parts.next()?.to_string();
    Some(StructuredDiagnostic {
        class: classify_rule(rule),
        code: Some(rule.to_string()),
        message: message.to_string(),
        path: Some(path),
        line: Some(line_no),
        column: Some(column),
        primary: true,
        suggestion: None,
    })
}

/// Preserve LSP `textDocument/publishDiagnostics` items as structured
/// diagnostics. `items` is the LSP `diagnostics` array; `path` the
/// document they belong to.
pub fn from_lsp_diagnostics(path: &str, items: &[serde_json::Value]) -> Vec<StructuredDiagnostic> {
    items
        .iter()
        .filter(|item| item.get("severity").and_then(|s| s.as_u64()).unwrap_or(1) == 1)
        .map(|item| {
            let rule = item
                .get("code")
                .and_then(|code| code.as_str())
                .map(str::to_string);
            StructuredDiagnostic {
                class: rule
                    .as_deref()
                    .map(classify_rule)
                    .unwrap_or(ResidualClass::Type),
                code: rule,
                message: item
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or_default()
                    .to_string(),
                path: Some(path.to_string()),
                line: item
                    .pointer("/range/start/line")
                    .and_then(|l| l.as_u64())
                    .and_then(|l| u32::try_from(l + 1).ok()),
                column: item
                    .pointer("/range/start/character")
                    .and_then(|c| c.as_u64())
                    .and_then(|c| u32::try_from(c + 1).ok()),
                primary: true,
                suggestion: None,
            }
        })
        .collect()
}
