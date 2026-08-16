//! The NASA coding rules Perspt is held to, and the vocabulary for reporting
//! where they are broken.
//!
//! Rule `NASA-2` is NASA/JPL *Power of Ten* Rule 4 — "no function longer than
//! what can be printed on a single sheet of paper" — relaxed from 60 lines to
//! this project's 70. `NASA-1` and `NASA-3` are Perspt constants.
//!
//! The rules apply to **Rust sources only**. Documentation, PSPs, and the
//! changelog are out of scope; see [`crate::scan`].

pub mod file_len;
pub mod fn_len;
pub mod line_len;

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A rule in the standard, identified by a stable code that appears in reports,
/// in the baseline file, and in CI output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RuleId {
    /// `NASA-1` — a source file must not exceed [`FILE_LINE_LIMIT`] lines.
    #[serde(rename = "NASA-1")]
    FileLength,
    /// `NASA-2` — a function must not exceed [`FUNCTION_LINE_LIMIT`] code lines.
    #[serde(rename = "NASA-2")]
    FunctionLength,
    /// `NASA-3` — a line must not exceed [`LINE_WIDTH_LIMIT`] columns.
    #[serde(rename = "NASA-3")]
    LineWidth,
}

/// `NASA-1`: maximum physical lines in one Rust source file.
pub const FILE_LINE_LIMIT: usize = 1408;

/// `NASA-2`: maximum code lines in one function, excluding comments and blanks.
pub const FUNCTION_LINE_LIMIT: usize = 70;

/// `NASA-3`: maximum columns in one line, counted in Unicode scalar values.
pub const LINE_WIDTH_LIMIT: usize = 108;

/// Every rule, in report order.
pub const ALL: [RuleId; 3] = [
    RuleId::FileLength,
    RuleId::FunctionLength,
    RuleId::LineWidth,
];

impl RuleId {
    /// The stable code used in reports, CI output, and the baseline file.
    pub fn code(self) -> &'static str {
        match self {
            RuleId::FileLength => "NASA-1",
            RuleId::FunctionLength => "NASA-2",
            RuleId::LineWidth => "NASA-3",
        }
    }

    /// The limit this rule enforces.
    pub fn limit(self) -> usize {
        match self {
            RuleId::FileLength => FILE_LINE_LIMIT,
            RuleId::FunctionLength => FUNCTION_LINE_LIMIT,
            RuleId::LineWidth => LINE_WIDTH_LIMIT,
        }
    }

    /// A one-line description, printed as the section heading of a report.
    pub fn title(self) -> &'static str {
        match self {
            RuleId::FileLength => "file length",
            RuleId::FunctionLength => "function length",
            RuleId::LineWidth => "line width",
        }
    }

    /// The unit the measurement is expressed in.
    pub fn unit(self) -> &'static str {
        match self {
            RuleId::FileLength | RuleId::FunctionLength => "lines",
            RuleId::LineWidth => "cols",
        }
    }

    /// Parse a rule code such as `NASA-2`, case-insensitively.
    pub fn parse(code: &str) -> Option<Self> {
        ALL.into_iter()
            .find(|r| r.code().eq_ignore_ascii_case(code))
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// One place where the standard is broken.
///
/// `line` anchors the violation for editor navigation: the first line of the
/// file for `NASA-1`, the `fn` keyword for `NASA-2`, and the offending line
/// itself for `NASA-3`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Violation {
    pub rule: RuleId,
    /// Repository-relative path, always with `/` separators.
    pub file: PathBuf,
    pub line: usize,
    /// The function name for `NASA-2`; `None` for file- and line-scoped rules.
    pub item: Option<String>,
    pub measured: usize,
    pub limit: usize,
}

impl Violation {
    /// How far past the limit this violation is.
    pub fn overage(&self) -> usize {
        self.measured.saturating_sub(self.limit)
    }

    /// `path:line` for `NASA-1`/`NASA-3`, `path:line fn name` for `NASA-2`.
    pub fn location(&self) -> String {
        let path = self.file.display();
        match &self.item {
            Some(name) => format!("{path}:{} fn {name}", self.line),
            None if self.rule == RuleId::FileLength => format!("{path}"),
            None => format!("{path}:{}", self.line),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_codes_round_trip() {
        for rule in ALL {
            assert_eq!(RuleId::parse(rule.code()), Some(rule));
        }
        assert_eq!(RuleId::parse("nasa-2"), Some(RuleId::FunctionLength));
        assert_eq!(RuleId::parse("NASA-9"), None);
    }

    #[test]
    fn limits_match_the_documented_standard() {
        assert_eq!(RuleId::FileLength.limit(), 1408);
        assert_eq!(RuleId::FunctionLength.limit(), 70);
        assert_eq!(RuleId::LineWidth.limit(), 108);
    }

    #[test]
    fn overage_is_the_distance_past_the_limit() {
        let v = Violation {
            rule: RuleId::FileLength,
            file: PathBuf::from("a.rs"),
            line: 1,
            item: None,
            measured: 5889,
            limit: 1408,
        };
        assert_eq!(v.overage(), 4481);
    }
}
