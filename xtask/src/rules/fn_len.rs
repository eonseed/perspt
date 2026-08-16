//! `PSP-2` — a function must not exceed 70 code lines.
//!
//! No function longer than what fits on a single sheet of paper — the
//! TigerBeetle-style ceiling Perspt adopted, set at 70 code lines.
//!
//! # Why this rule parses instead of counting braces
//!
//! A brace-counting heuristic measures
//! `crates/perspt-tui/src/chat_app.rs::strip_latex_wrappers` at **2,271
//! lines**. It is roughly forty. The body is an array of literals —
//! `"\\mathbf{"`, `"\\text{"`, `"\\operatorname{"` — and the unbalanced braces
//! *inside string literals* defeat every text-based counter. A compliance tool
//! that cries wolf gets switched off, so this rule uses the real grammar.
//!
//! A line counts when it carries at least one token and is not a comment.
//! Blank lines, `//` comments, doc comments, and `/* */` blocks are excluded;
//! the signature and the closing brace are included, because they are part of
//! what a reader must hold in their head.
//!
//! Closures count toward their enclosing function. Nested `fn` items do not —
//! they are measured separately, and their lines are subtracted from the
//! enclosing function so that one long body cannot hide inside another.

use anyhow::{Context, Result};
use proc_macro2::{Span, TokenStream, TokenTree};
use syn::visit::{self, Visit};

use crate::rules::{RuleId, Violation, FUNCTION_LINE_LIMIT};
use crate::scan::SourceFile;

/// One function's name and the line range it occupies, inclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FnSpan {
    name: String,
    start: usize,
    end: usize,
}

impl FnSpan {
    /// Whether `other` lies strictly inside this span.
    fn contains(&self, other: &FnSpan) -> bool {
        self.start <= other.start && other.end <= self.end && self != other
    }
}

/// Check one file, returning one violation per oversized function.
pub fn check(file: &SourceFile) -> Result<Vec<Violation>> {
    let total = file.line_count();
    let stream: TokenStream = file
        .text
        .parse()
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| format!("tokenizing {}", file.rel.display()))?;

    let mut carries_token = vec![false; total];
    mark_token_lines(stream, &mut carries_token);
    let is_comment = comment_lines(file);

    let ast =
        syn::parse_file(&file.text).with_context(|| format!("parsing {}", file.rel.display()))?;
    let mut collector = FnCollector::default();
    collector.visit_file(&ast);

    Ok(measure(&collector.spans, &carries_token, &is_comment)
        .into_iter()
        .filter(|(_, measured)| *measured > FUNCTION_LINE_LIMIT)
        .map(|(span, measured)| Violation {
            rule: RuleId::FunctionLength,
            file: file.rel.clone(),
            line: span.start,
            item: Some(span.name),
            measured,
            limit: FUNCTION_LINE_LIMIT,
        })
        .collect())
}

/// Count each function's code lines, excluding any nested function's lines.
fn measure(spans: &[FnSpan], carries_token: &[bool], is_comment: &[bool]) -> Vec<(FnSpan, usize)> {
    spans
        .iter()
        .map(|span| {
            let nested: Vec<&FnSpan> = spans.iter().filter(|other| span.contains(other)).collect();
            let count = (span.start..=span.end)
                .filter(|line| {
                    let index = line - 1;
                    carries_token.get(index).copied().unwrap_or(false)
                        && !is_comment.get(index).copied().unwrap_or(false)
                        && !nested.iter().any(|n| n.start <= *line && *line <= n.end)
                })
                .count();
            (span.clone(), count)
        })
        .collect()
}

/// Mark every line carrying at least one token.
///
/// Iterative by design: a recursive walk would put this tool in violation of
/// *Power of Ten* Rule 1, which is a poor look for the thing enforcing it.
/// Group delimiters are marked individually rather than by the group's own
/// span, which would otherwise cover — and wrongly mark — every comment line
/// inside the block.
fn mark_token_lines(stream: TokenStream, marks: &mut [bool]) {
    let mut stack = vec![stream.into_iter()];
    while let Some(frame) = stack.last_mut() {
        match frame.next() {
            None => {
                stack.pop();
            }
            Some(TokenTree::Group(group)) => {
                mark_span(group.span_open(), marks);
                mark_span(group.span_close(), marks);
                stack.push(group.stream().into_iter());
            }
            Some(other) => mark_span(other.span(), marks),
        }
    }
}

/// Mark every line a single span covers; a raw string may cover several.
fn mark_span(span: Span, marks: &mut [bool]) {
    let (first, last) = (span.start().line, span.end().line);
    for line in first..=last {
        if let Some(slot) = marks.get_mut(line.saturating_sub(1)) {
            *slot = true;
        }
    }
}

/// Flag lines whose content is only a `//`, `///`, or `//!` comment.
///
/// Block comments need no handling: they emit no tokens, so they are already
/// excluded by [`mark_token_lines`].
fn comment_lines(file: &SourceFile) -> Vec<bool> {
    file.lines()
        .iter()
        .map(|line| line.trim_start().starts_with("//"))
        .collect()
}

/// Collects free functions, inherent and trait-impl methods, and trait methods
/// that carry a default body.
#[derive(Default)]
struct FnCollector {
    spans: Vec<FnSpan>,
}

impl FnCollector {
    fn push(&mut self, name: String, sig: &syn::Signature, brace: &syn::token::Brace) {
        self.spans.push(FnSpan {
            name,
            start: sig.fn_token.span.start().line,
            end: brace.span.close().end().line,
        });
    }
}

impl<'ast> Visit<'ast> for FnCollector {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.push(
            node.sig.ident.to_string(),
            &node.sig,
            &node.block.brace_token,
        );
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.push(
            node.sig.ident.to_string(),
            &node.sig,
            &node.block.brace_token,
        );
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        if let Some(body) = &node.default {
            self.push(node.sig.ident.to_string(), &node.sig, &body.brace_token);
        }
        visit::visit_trait_item_fn(self, node);
    }
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

    /// Measure every function in `text`, keyed by name.
    fn measured(text: &str) -> Vec<(String, usize)> {
        let file = source(text);
        let stream: TokenStream = file.text.parse().expect("tokenizes");
        let mut marks = vec![false; file.line_count()];
        mark_token_lines(stream, &mut marks);
        let comments = comment_lines(&file);
        let ast = syn::parse_file(&file.text).expect("parses");
        let mut collector = FnCollector::default();
        collector.visit_file(&ast);
        measure(&collector.spans, &marks, &comments)
            .into_iter()
            .map(|(span, count)| (span.name, count))
            .collect()
    }

    #[test]
    fn string_literals_with_unbalanced_braces_do_not_inflate_the_count() {
        // The regression that motivates parsing: brace counting reports the
        // real `strip_latex_wrappers` as 2,271 lines.
        let text = r#"
fn strip(text: &str) -> String {
    let wrappers = ["\\mathbf{", "\\text{", "\\operatorname{"];
    let _ = wrappers;
    text.to_string()
}
"#;
        assert_eq!(measured(text), [("strip".to_string(), 5)]);
    }

    #[test]
    fn comments_and_blank_lines_are_excluded() {
        let text = r#"
fn f() {
    // a line comment

    /// not really a doc comment here, but still a comment
    let a = 1;

    /* block
       comment */
    let _ = a;
}
"#;
        // Counted: `fn f() {`, `let a = 1;`, `let _ = a;`, `}`.
        assert_eq!(measured(text), [("f".to_string(), 4)]);
    }

    #[test]
    fn a_multi_line_signature_is_part_of_the_function() {
        let text = r#"
fn wide(
    a: u32,
    b: u32,
) -> u32 {
    a + b
}
"#;
        assert_eq!(measured(text), [("wide".to_string(), 6)]);
    }

    #[test]
    fn a_nested_function_is_measured_separately_and_subtracted() {
        let text = r#"
fn outer() {
    fn inner() {
        let a = 1;
        let _ = a;
    }
    inner();
}
"#;
        let found = measured(text);
        // `outer` keeps only `fn outer() {`, `inner();`, `}` — inner's four
        // lines belong to inner.
        assert!(found.contains(&("outer".to_string(), 3)), "{found:?}");
        assert!(found.contains(&("inner".to_string(), 4)), "{found:?}");
    }

    #[test]
    fn closures_count_toward_the_enclosing_function() {
        let text = r#"
fn host() {
    let f = |x: u32| {
        x + 1
    };
    let _ = f(1);
}
"#;
        assert_eq!(measured(text), [("host".to_string(), 6)]);
    }

    #[test]
    fn methods_and_defaulted_trait_methods_are_measured() {
        let text = r#"
trait T {
    fn required(&self) -> u32;
    fn defaulted(&self) -> u32 {
        1
    }
}
struct S;
impl S {
    fn method(&self) -> u32 {
        2
    }
}
"#;
        let found = measured(text);
        let names: Vec<&str> = found.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"defaulted"), "{names:?}");
        assert!(names.contains(&"method"), "{names:?}");
        assert!(
            !names.contains(&"required"),
            "a bodiless method has no length"
        );
    }

    #[test]
    fn a_function_at_the_limit_passes_and_one_line_over_fails() {
        let body = "    let _x = 1;\n".repeat(FUNCTION_LINE_LIMIT - 2);
        let at_limit = format!("fn f() {{\n{body}}}\n");
        assert!(check(&source(&at_limit)).expect("checks").is_empty());

        let over = format!("fn f() {{\n{body}    let _y = 2;\n}}\n");
        let found = check(&source(&over)).expect("checks");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].measured, FUNCTION_LINE_LIMIT + 1);
        assert_eq!(found[0].item.as_deref(), Some("f"));
    }
}
