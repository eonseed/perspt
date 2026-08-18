//! `tsc --pretty false` diagnostics (PSP-10 system 26).
//!
//! A versioned normalizer over the compiler's plain text form
//! `path(line,col): error TSxxxx: message`. The PSP does not claim a JSON
//! diagnostic protocol for `tsc`.

use super::types::StructuredDiagnostic;
use crate::lang::classify_ts_code;

/// Parser identity; enters the sensor fingerprint.
pub const PARSER_ID: &str = "tsc-text-v1";

/// Parse `tsc --pretty false` error lines.
pub fn parse(raw: &str) -> Vec<StructuredDiagnostic> {
    raw.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<StructuredDiagnostic> {
    // `src/index.ts(4,7): error TS2322: Type 'string' is not assignable...`
    let marker = line.find(": error TS")?;
    let (location, rest) = line.split_at(marker);
    let rest = &rest[": error ".len()..];
    let (code, message) = rest.split_once(": ")?;
    let (path, position) = location.rsplit_once('(')?;
    let position = position.strip_suffix(')')?;
    let (line_no, column) = position.split_once(',')?;
    Some(StructuredDiagnostic {
        class: classify_ts_code(code),
        code: Some(code.to_string()),
        message: message.to_string(),
        path: Some(path.to_string()),
        line: line_no.trim().parse().ok(),
        column: column.trim().parse().ok(),
        primary: true,
        suggestion: None,
    })
}
