//! `NASA-1` — a Rust source file must not exceed 1408 physical lines.
//!
//! Every line counts, including comments and blanks: this is a file-size rule,
//! not a code-density rule. A file that outgrows the limit is asking to be
//! split along the seam it already has.

use crate::rules::{RuleId, Violation, FILE_LINE_LIMIT};
use crate::scan::SourceFile;

/// Check one file, returning at most one violation.
pub fn check(file: &SourceFile) -> Option<Violation> {
    let measured = file.line_count();
    if measured <= FILE_LINE_LIMIT {
        return None;
    }
    Some(Violation {
        rule: RuleId::FileLength,
        file: file.rel.clone(),
        line: 1,
        item: None,
        measured,
        limit: FILE_LINE_LIMIT,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(lines: usize) -> SourceFile {
        SourceFile {
            rel: PathBuf::from("a.rs"),
            text: "x\n".repeat(lines),
        }
    }

    #[test]
    fn a_file_at_the_limit_passes() {
        assert!(check(&source(FILE_LINE_LIMIT)).is_none());
    }

    #[test]
    fn a_file_one_line_over_fails_with_its_overage() {
        let violation = check(&source(FILE_LINE_LIMIT + 1)).expect("must flag");
        assert_eq!(violation.measured, FILE_LINE_LIMIT + 1);
        assert_eq!(violation.overage(), 1);
        assert_eq!(violation.line, 1);
    }
}
