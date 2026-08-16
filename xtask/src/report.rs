//! Rendering violations for humans, for pull requests, and for machines.

use std::fmt::Write as _;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::rules::{self, RuleId, Violation};

/// How a report is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// Aligned plain text, grouped by rule. The default.
    Table,
    /// A single JSON document, for dashboards and scripts.
    Json,
    /// GitHub-flavoured Markdown, for pasting into a pull request.
    Markdown,
}

/// The shape emitted by [`Format::Json`].
#[derive(Debug, Serialize)]
struct JsonReport<'a> {
    total: usize,
    by_rule: Vec<JsonRuleSummary>,
    violations: &'a [Violation],
}

#[derive(Debug, Serialize)]
struct JsonRuleSummary {
    rule: RuleId,
    code: &'static str,
    title: &'static str,
    limit: usize,
    count: usize,
}

/// Render `violations` in the requested format.
pub fn render(violations: &[Violation], format: Format) -> Result<String> {
    match format {
        Format::Table => Ok(render_table(violations)),
        Format::Markdown => Ok(render_markdown(violations)),
        Format::Json => render_json(violations),
    }
}

/// Violations of `rule`, in report order.
fn of_rule(violations: &[Violation], rule: RuleId) -> Vec<&Violation> {
    let mut found: Vec<&Violation> = violations.iter().filter(|v| v.rule == rule).collect();
    found.sort_by(|a, b| {
        b.measured
            .cmp(&a.measured)
            .then_with(|| a.file.cmp(&b.file))
    });
    found
}

fn render_table(violations: &[Violation]) -> String {
    if violations.is_empty() {
        return "All NASA coding rules pass.\n".to_string();
    }
    let mut out = String::new();
    for rule in rules::ALL {
        let found = of_rule(violations, rule);
        if found.is_empty() {
            continue;
        }
        let _ = writeln!(out, "{}  {} > {}", rule.code(), rule.title(), rule.limit());
        let width = found.iter().map(|v| v.location().len()).max().unwrap_or(0);
        for violation in &found {
            let _ = writeln!(
                out,
                "  {:<width$}  {:>6} {}  (+{})",
                violation.location(),
                violation.measured,
                rule.unit(),
                violation.overage(),
            );
        }
        out.push('\n');
    }
    out.push_str(&summary_line(violations));
    out
}

fn render_markdown(violations: &[Violation]) -> String {
    if violations.is_empty() {
        return "**All NASA coding rules pass.**\n".to_string();
    }
    let mut out = String::from("## NASA coding rules\n\n");
    for rule in rules::ALL {
        let found = of_rule(violations, rule);
        if found.is_empty() {
            continue;
        }
        let _ = writeln!(
            out,
            "### `{}` — {} > {}\n",
            rule.code(),
            rule.title(),
            rule.limit()
        );
        out.push_str("| Location | Measured | Over |\n|---|---:|---:|\n");
        for violation in &found {
            let _ = writeln!(
                out,
                "| `{}` | {} {} | +{} |",
                violation.location(),
                violation.measured,
                rule.unit(),
                violation.overage(),
            );
        }
        out.push('\n');
    }
    let _ = writeln!(out, "{}", summary_line(violations).trim_end());
    out
}

fn render_json(violations: &[Violation]) -> Result<String> {
    let by_rule = rules::ALL
        .into_iter()
        .map(|rule| JsonRuleSummary {
            rule,
            code: rule.code(),
            title: rule.title(),
            limit: rule.limit(),
            count: violations.iter().filter(|v| v.rule == rule).count(),
        })
        .collect();
    let report = JsonReport {
        total: violations.len(),
        by_rule,
        violations,
    };
    serde_json::to_string_pretty(&report).context("serializing the JSON report")
}

/// A one-line tally, e.g. `8 violations: NASA-1 x8`.
fn summary_line(violations: &[Violation]) -> String {
    let parts: Vec<String> = rules::ALL
        .into_iter()
        .filter_map(|rule| {
            let count = violations.iter().filter(|v| v.rule == rule).count();
            (count > 0).then(|| format!("{} x{count}", rule.code()))
        })
        .collect();
    format!("{} violations: {}\n", violations.len(), parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn violation(rule: RuleId, file: &str, measured: usize) -> Violation {
        Violation {
            rule,
            file: PathBuf::from(file),
            line: 1,
            item: None,
            measured,
            limit: rule.limit(),
        }
    }

    #[test]
    fn a_clean_tree_says_so_in_every_format() {
        assert!(render(&[], Format::Table).unwrap().contains("pass"));
        assert!(render(&[], Format::Markdown).unwrap().contains("pass"));
        assert!(render(&[], Format::Json).unwrap().contains("\"total\": 0"));
    }

    #[test]
    fn the_table_names_location_measurement_and_overage() {
        let found = [violation(RuleId::FileLength, "crates/a/src/big.rs", 5889)];
        let text = render(&found, Format::Table).unwrap();
        assert!(text.contains("NASA-1"), "{text}");
        assert!(text.contains("crates/a/src/big.rs"), "{text}");
        assert!(text.contains("5889 lines"), "{text}");
        assert!(text.contains("(+4481)"), "{text}");
    }

    #[test]
    fn the_worst_offender_is_listed_first() {
        let found = [
            violation(RuleId::FileLength, "small.rs", 1500),
            violation(RuleId::FileLength, "huge.rs", 5889),
        ];
        let text = render(&found, Format::Table).unwrap();
        let huge = text.find("huge.rs").expect("listed");
        let small = text.find("small.rs").expect("listed");
        assert!(huge < small, "{text}");
    }

    #[test]
    fn a_function_violation_carries_its_name() {
        let mut found = violation(RuleId::FunctionLength, "crates/a/src/x.rs", 465);
        found.line = 31;
        found.item = Some("step_verify".to_string());
        let text = render(&[found], Format::Table).unwrap();
        assert!(
            text.contains("crates/a/src/x.rs:31 fn step_verify"),
            "{text}"
        );
    }

    #[test]
    fn json_is_valid_and_reports_per_rule_counts() {
        let found = [
            violation(RuleId::FileLength, "a.rs", 2000),
            violation(RuleId::LineWidth, "a.rs", 120),
        ];
        let text = render(&found, Format::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(parsed["total"], 2);
        assert_eq!(parsed["by_rule"][0]["code"], "NASA-1");
        assert_eq!(parsed["by_rule"][0]["count"], 1);
    }

    #[test]
    fn the_summary_tallies_each_rule() {
        let found = [
            violation(RuleId::LineWidth, "a.rs", 120),
            violation(RuleId::LineWidth, "b.rs", 130),
        ];
        assert_eq!(summary_line(&found), "2 violations: NASA-3 x2\n");
    }
}
