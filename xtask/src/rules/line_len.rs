//! `NASA-3` — a line must not exceed 108 columns.
//!
//! Width is counted in Unicode scalar values, not bytes, so a line carrying
//! CJK text or an emoji is measured by what a reader sees rather than by its
//! UTF-8 encoding. `沈黛君` is three columns here, not nine.
//!
//! rustfmt runs at its default width of 100, so formatted code sits well
//! inside this ceiling. What this rule actually catches is the long string
//! literal or trailing comment rustfmt cannot break for you.

use crate::rules::{RuleId, Violation, LINE_WIDTH_LIMIT};
use crate::scan::SourceFile;

/// Check one file, returning one violation per offending line.
pub fn check(file: &SourceFile) -> Vec<Violation> {
    file.text
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let measured = line.chars().count();
            if measured <= LINE_WIDTH_LIMIT {
                return None;
            }
            Some(Violation {
                rule: RuleId::LineWidth,
                file: file.rel.clone(),
                line: index + 1,
                item: None,
                measured,
                limit: LINE_WIDTH_LIMIT,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(text: &str) -> SourceFile {
        SourceFile {
            rel: PathBuf::from("a.rs"),
            text: text.to_string(),
        }
    }

    #[test]
    fn a_line_at_the_limit_passes() {
        assert!(check(&source(&"x".repeat(LINE_WIDTH_LIMIT))).is_empty());
    }

    #[test]
    fn each_offending_line_is_reported_once_with_its_number() {
        let long = "x".repeat(LINE_WIDTH_LIMIT + 5);
        let text = format!("short\n{long}\nshort\n{long}\n");
        let found = check(&source(&text));
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].line, 2);
        assert_eq!(found[1].line, 4);
        assert_eq!(found[0].overage(), 5);
    }

    #[test]
    fn width_is_counted_in_characters_not_bytes() {
        // 108 CJK characters are 324 UTF-8 bytes but exactly 108 columns.
        let cjk = "沈".repeat(LINE_WIDTH_LIMIT);
        assert_eq!(cjk.len(), LINE_WIDTH_LIMIT * 3);
        assert!(check(&source(&cjk)).is_empty());
    }
}
