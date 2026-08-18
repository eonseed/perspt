//! Pytest JUnit XML results (PSP-10 system 26): test identity preserved.
//!
//! The parser targets exactly the JUnit shape pytest emits
//! (`--junit-xml`): `<testcase classname=".." name="..">` elements whose
//! body contains `<failure` or `<error`. It is a constrained hand-rolled
//! scan, fixture-tested; a general XML dependency buys nothing this
//! grammar needs.

use perspt_sdk::ResidualClass;

use super::types::StructuredDiagnostic;

/// Parser identity; enters the sensor fingerprint.
pub const PARSER_ID: &str = "pytest-junit-v1";

/// Parse failed test cases from a JUnit XML report.
pub fn parse(raw: &str) -> Vec<StructuredDiagnostic> {
    let mut failures = Vec::new();
    let mut rest = raw;
    while let Some(start) = rest.find("<testcase") {
        rest = &rest[start..];
        let Some(tag_end) = rest.find('>') else { break };
        let tag = &rest[..tag_end + 1];
        let self_closing = tag.ends_with("/>");
        let body_end = if self_closing {
            tag_end + 1
        } else {
            match rest.find("</testcase>") {
                Some(end) => end,
                None => break,
            }
        };
        let body = &rest[tag_end + 1..body_end];
        if !self_closing && (body.contains("<failure") || body.contains("<error")) {
            let classname = attribute(tag, "classname").unwrap_or_default();
            let name = attribute(tag, "name").unwrap_or_default();
            let test_id = if classname.is_empty() {
                name.clone()
            } else {
                format!("{classname}::{name}")
            };
            failures.push(StructuredDiagnostic {
                class: ResidualClass::TestFailure,
                code: Some(test_id.clone()),
                message: failure_message(body).unwrap_or_else(|| format!("{test_id} failed")),
                path: attribute(tag, "file"),
                line: attribute(tag, "line").and_then(|line| line.parse().ok()),
                column: None,
                primary: true,
                suggestion: None,
            });
        }
        rest = &rest[body_end..];
    }
    failures
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    // Whitespace before the name keeps `name=` from matching inside
    // `classname=`.
    let marker = format!("{name}=\"");
    let mut search = 0;
    loop {
        let found = tag[search..].find(&marker)? + search;
        let boundary = found == 0 || tag[..found].ends_with(char::is_whitespace);
        if boundary {
            let start = found + marker.len();
            let end = tag[start..].find('"')? + start;
            return Some(unescape(&tag[start..end]));
        }
        search = found + marker.len();
    }
}

fn failure_message(body: &str) -> Option<String> {
    let start = body.find("<failure").or_else(|| body.find("<error"))?;
    let tag = &body[start..];
    attribute(&tag[..tag.find('>')? + 1], "message")
}

fn unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}
