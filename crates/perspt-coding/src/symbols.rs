//! Symbol extraction for the goal-presence sensor (PSP-8).
//!
//! The SDK's [`perspt_sdk::goal`] sensor compares *names*: the symbols a node is
//! required to produce against the symbols actually defined in the workspace.
//! Turning a coding task contract into expected names, and source files into
//! defined names, is domain knowledge — it lives here, not in the kernel.
//!
//! The extractors are deliberately lightweight, language-shared scanners over
//! Rust, Python, and TypeScript declaration keywords plus backtick-quoted
//! identifiers in a goal description. They are intentionally *conservative*:
//! they only ever name top-level declarable identifiers, so the goal-presence
//! sensor never invents an obligation the planner did not express.

use std::collections::BTreeSet;

/// Declaration keywords across the supported languages whose following token is
/// a defined symbol name.
const DECL_KEYWORDS: &[&str] = &[
    "fn", "def", "function", "struct", "enum", "trait", "class", "interface", "type",
];

/// True for an identifier start character (ASCII letter or underscore).
fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

/// True for an identifier continuation character.
fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Extract the identifier that begins at `chars[i..]`, returning it and the
/// index just past it. Assumes `chars[i]` is an identifier start.
fn take_ident(chars: &[char], i: usize) -> (String, usize) {
    let mut j = i;
    while j < chars.len() && is_ident_continue(chars[j]) {
        j += 1;
    }
    (chars[i..j].iter().collect(), j)
}

/// Names declared in `source` via a `DECL_KEYWORDS` keyword.
///
/// Scans token by token: every time a declaration keyword is seen as a whole
/// word, the next identifier token is recorded as a defined symbol. This catches
/// `pub fn multiply(...)`, `def is_even(n):`, `export function f()`,
/// `struct Foo`, `class Bar`, etc. without a full parser.
pub fn defined_symbols(source: &str) -> BTreeSet<String> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = BTreeSet::new();
    let mut i = 0;
    while i < chars.len() {
        if is_ident_start(chars[i]) {
            // Word boundary before: previous char must not be ident-continue.
            let at_boundary = i == 0 || !is_ident_continue(chars[i - 1]);
            let (word, next) = take_ident(&chars, i);
            if at_boundary && DECL_KEYWORDS.contains(&word.as_str()) {
                // Skip whitespace, then take the declared name.
                let mut k = next;
                while k < chars.len() && chars[k].is_whitespace() {
                    k += 1;
                }
                if k < chars.len() && is_ident_start(chars[k]) {
                    let (name, end) = take_ident(&chars, k);
                    out.insert(name);
                    i = end;
                    continue;
                }
            }
            i = next;
        } else {
            i += 1;
        }
    }
    out
}

/// Names the goal requires to exist, drawn from a declared interface signature
/// and the natural-language goal text.
///
/// Two sources, both conservative:
/// 1. `interface_signature` — the contract's declared public API. Declaration
///    keywords there name required symbols directly.
/// 2. `goal` — backtick-quoted identifiers (``` `multiply` ```), and identifiers
///    immediately followed by `(` (a call/definition shape, e.g.
///    `` `is_even(n: i32)` ``). Prose words are ignored unless they carry one of
///    those code shapes, so a chatty goal does not manufacture obligations.
pub fn expected_symbols(interface_signature: &str, goal: &str) -> Vec<String> {
    let mut ordered: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    let mut push = |name: String| {
        if name.len() >= 2 && is_ident_start(name.chars().next().unwrap()) && seen.insert(name.clone())
        {
            ordered.push(name);
        }
    };

    // 1. Declared interface signature: declaration-keyword names.
    for name in defined_symbols(interface_signature) {
        push(name);
    }

    // 2. Goal text: backtick spans and `ident(` call shapes.
    for span in backtick_spans(goal) {
        // Inside a span, capture both declaration-keyword names and `ident(`.
        for name in defined_symbols(&span) {
            push(name);
        }
        for name in call_shaped_idents(&span) {
            push(name);
        }
        // A span that is *just* a bare identifier (e.g. "implement `lcm`") names
        // a required symbol directly — unless it is a primitive type or language
        // keyword that prose routinely quotes (e.g. `i32`, `bool`).
        let trimmed = span.trim();
        if is_bare_identifier(trimmed) && !is_primitive_or_keyword(trimmed) {
            push(trimmed.to_string());
        }
    }
    // Also catch un-quoted `ident(` shapes in the bare goal text.
    for name in call_shaped_idents(goal) {
        push(name);
    }

    ordered
}

/// True when `s` is a single bare identifier (ident-start then ident-continue,
/// nothing else) — so `lcm` qualifies but `src/lib.rs`, `i32-based`, and
/// `fn foo` do not.
fn is_bare_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if is_ident_start(c) => chars.all(is_ident_continue),
        _ => false,
    }
}

/// Primitive types and language keywords that appear back-ticked in prose but
/// are never the *symbol the goal asks to create*. Kept deliberately small and
/// cross-language (Rust / Python / TypeScript) so the goal-presence sensor does
/// not manufacture an obligation for a type name.
fn is_primitive_or_keyword(name: &str) -> bool {
    const DENY: &[&str] = &[
        // Rust integer / float / core scalar types.
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        "f32", "f64", "bool", "char", "str", "String", "Vec", "Option", "Result", "Box", "Self",
        // Common literals / keywords.
        "self", "true", "false", "None", "Some", "Ok", "Err", "fn", "def", "function", "struct",
        "enum", "trait", "class", "interface", "type", "pub", "let", "const", "mut", "async",
        "await", "return", "if", "else", "for", "while", "match",
        // TypeScript / Python primitives.
        "number", "string", "boolean", "void", "any", "unknown", "int", "float", "double", "long",
        "short", "byte", "object", "null", "undefined",
    ];
    DENY.contains(&name)
}

/// The contents of every back-tick delimited span in `text`.
fn backtick_spans(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut current: Option<String> = None;
    for c in text.chars() {
        if c == '`' {
            match current.take() {
                Some(s) => spans.push(s),
                None => current = Some(String::new()),
            }
        } else if let Some(buf) = current.as_mut() {
            buf.push(c);
        }
    }
    spans
}

/// Identifiers *immediately* followed by `(`, e.g. the `is_even` in
/// `is_even(n: i32)`. Whitespace before the paren is NOT allowed, so prose like
/// "Reverse Polish Notation (RPN)" does not misread "Notation" as a call.
/// Keywords are excluded so control-flow words like `if(` are never symbols.
fn call_shaped_idents(text: &str) -> Vec<String> {
    const NOISE: &[&str] = &[
        "if", "for", "while", "match", "switch", "return", "fn", "def", "function",
    ];
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if is_ident_start(chars[i]) && (i == 0 || !is_ident_continue(chars[i - 1])) {
            let (name, next) = take_ident(&chars, i);
            if next < chars.len() && chars[next] == '(' && !NOISE.contains(&name.as_str()) {
                out.push(name);
            }
            i = next;
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn rust_defined_symbols() {
        let src = "pub fn multiply(a: i32, b: i32) -> i32 { a * b }\nstruct Pair { a: i32 }";
        assert_eq!(defined_symbols(src), set(&["multiply", "Pair"]));
    }

    #[test]
    fn python_defined_symbols() {
        let src = "def is_even(n):\n    return n % 2 == 0\nclass Calc:\n    pass";
        assert_eq!(defined_symbols(src), set(&["is_even", "Calc"]));
    }

    #[test]
    fn typescript_defined_symbols() {
        let src = "export function add(a: number, b: number) { return a + b }\ninterface Shape {}";
        assert_eq!(defined_symbols(src), set(&["add", "Shape"]));
    }

    #[test]
    fn placeholder_file_defines_nothing() {
        assert!(defined_symbols("// implement here\n").is_empty());
    }

    #[test]
    fn keyword_substring_is_not_a_declaration() {
        // `define` contains `def` but is not the `def` keyword.
        assert!(defined_symbols("define_macro_helper = 1").is_empty());
    }

    #[test]
    fn expected_from_interface_signature() {
        let expected = expected_symbols("pub fn is_even(n: i32) -> bool", "");
        assert_eq!(expected, vec!["is_even"]);
    }

    #[test]
    fn expected_from_backticked_goal() {
        let expected = expected_symbols(
            "",
            "Add a public function `multiply(a: i32, b: i32) -> i32` that returns a*b.",
        );
        assert_eq!(expected, vec!["multiply"]);
    }

    #[test]
    fn expected_from_call_shape_in_goal() {
        let expected = expected_symbols("", "Implement is_even(n) returning true for even n.");
        assert_eq!(expected, vec!["is_even"]);
    }

    #[test]
    fn prose_goal_yields_no_false_obligation() {
        let expected =
            expected_symbols("", "Refactor the module for clarity and improve the docs.");
        assert!(expected.is_empty());
    }

    #[test]
    fn control_flow_words_are_not_symbols() {
        let expected = expected_symbols("", "if (x) do something; while (y) loop.");
        assert!(expected.is_empty());
    }

    #[test]
    fn prose_word_before_spaced_paren_is_not_a_symbol() {
        // Regression: "Reverse Polish Notation (RPN)" must not yield "Notation"
        // — a space before '(' means it is prose, not a call.
        let expected = expected_symbols(
            "",
            "Build a Reverse Polish Notation (RPN) calculator library.",
        );
        assert!(expected.is_empty(), "got {expected:?}");
    }

    #[test]
    fn bare_backtick_identifier_is_expected() {
        // The case that slipped through: "Implement `lcm`" with no parens.
        let expected = expected_symbols("", "Implement `lcm` in src/lib.rs with a unit test.");
        assert_eq!(expected, vec!["lcm"]);
    }

    #[test]
    fn backticked_primitive_type_is_not_an_obligation() {
        // `i32` and `src/lib.rs` are quoted prose, not symbols to create.
        let expected =
            expected_symbols("", "Use an `i32`-based signature; write to `src/lib.rs`.");
        assert!(expected.is_empty(), "got {expected:?}");
    }

    #[test]
    fn is_bare_identifier_rejects_paths_and_snippets() {
        assert!(is_bare_identifier("lcm"));
        assert!(is_bare_identifier("is_even"));
        assert!(!is_bare_identifier("src/lib.rs"));
        assert!(!is_bare_identifier("fn foo"));
        assert!(!is_bare_identifier("a*b"));
        assert!(!is_bare_identifier(""));
    }
}
