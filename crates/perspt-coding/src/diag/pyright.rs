//! Pyright diagnostics — the declared fallback Python sensor
//! (PSP-10 system 26).
//!
//! Pyright is used only when `ty` is unavailable, under its own distinct
//! fingerprint; measurements from `ty` and Pyright are never pooled as one
//! calibration stratum.

use perspt_sdk::ResidualClass;

use super::types::StructuredDiagnostic;

/// Parser identity; enters the sensor fingerprint.
pub const PARSER_ID: &str = "pyright-json-v1";

fn classify_rule(rule: &str) -> ResidualClass {
    match rule {
        "reportMissingImports" | "reportMissingModuleSource" => ResidualClass::ImportGraph,
        "reportUndefinedVariable" | "reportAttributeAccessIssue" => ResidualClass::SymbolMismatch,
        "reportAssignmentType"
        | "reportArgumentType"
        | "reportReturnType"
        | "reportGeneralTypeIssues"
        | "reportOperatorIssue" => ResidualClass::Type,
        _ => ResidualClass::Type,
    }
}

/// Parse `pyright --outputjson` output.
pub fn parse(raw: &str) -> Vec<StructuredDiagnostic> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };
    let Some(items) = value.get("generalDiagnostics").and_then(|d| d.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter(|item| item.get("severity").and_then(|s| s.as_str()) == Some("error"))
        .map(|item| {
            let rule = item
                .get("rule")
                .and_then(|rule| rule.as_str())
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
                path: item
                    .get("file")
                    .and_then(|f| f.as_str())
                    .map(str::to_string),
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
