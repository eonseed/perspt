//! Chat Application for Perspt TUI
//!
//! An elegant chat interface with markdown rendering, syntax highlighting,
//! and reliable key handling. Now with async event-driven architecture.

use crate::app_event::AppEvent;
use crate::simple_input::SimpleInput;
use crate::theme::icons;
use anyhow::Result;
use crossterm::event::{
    Event as CrosstermEvent, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind,
};
use perspt_core::{GenAIProvider, EOT_SIGNAL};
use ratatui::{
    crossterm::event::{self, Event},
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    DefaultTerminal, Frame,
};
use std::sync::Arc;
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::mpsc;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Role of a chat message
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Alignment of a table column
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlign {
    Left,
    Center,
    Right,
}

/// A block of markdown content, parsed to separate tables from other markdown text
#[derive(Debug, Clone)]
pub enum ContentBlock {
    Markdown(String),
    Table {
        headers: Vec<String>,
        alignments: Vec<TableAlign>,
        rows: Vec<Vec<String>>,
    },
}

/// A single chat message
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub reasoning: Option<String>,
    pub cached_visual_lines: Vec<Line<'static>>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            reasoning: None,
            cached_visual_lines: Vec::new(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            reasoning: None,
            cached_visual_lines: Vec::new(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            reasoning: None,
            cached_visual_lines: Vec::new(),
        }
    }

    /// Parse thinking blocks from content.
    /// Returns (thought_content, remaining_content)
    pub fn parse_inline_thought(content: &str) -> (Option<String>, String) {
        if let Some(start_idx) = content.find("<think>") {
            if let Some(end_idx) = content.find("</think>") {
                if end_idx > start_idx {
                    let thought = content[start_idx + "<think>".len()..end_idx].to_string();
                    let remaining = format!(
                        "{}{}",
                        &content[..start_idx],
                        &content[end_idx + "</think>".len()..]
                    );
                    return (Some(thought), remaining);
                }
            } else {
                // Unclosed <think> tag (still streaming)
                let thought = content[start_idx + "<think>".len()..].to_string();
                let remaining = content[..start_idx].to_string();
                return (Some(thought), remaining);
            }
        }
        (None, content.to_string())
    }

    /// Transpile simple superscripts like ^2 to ² (only single standalone characters)
    fn replace_superscripts(text: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '^' && i + 1 < chars.len() {
                let next_c = chars[i + 1];
                // Only convert if the char after the candidate is NOT a letter
                // (so we don't convert ^i from ^int or ^max)
                let after_is_letter = i + 2 < chars.len() && chars[i + 2].is_ascii_alphabetic();
                let is_letter_candidate = next_c.is_ascii_alphabetic();
                if is_letter_candidate && after_is_letter {
                    // Part of a multi-char name like ^{max} → ^max, keep as-is
                    result.push('^');
                    i += 1;
                    continue;
                }
                let super_c = match next_c {
                    '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴',
                    '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹',
                    '+' => '⁺', '-' => '⁻', '=' => '⁼', '(' => '⁽', ')' => '⁾',
                    'n' => 'ⁿ', 'i' => 'ⁱ', 'x' => 'ˣ', 'y' => 'ʸ',
                    _ => next_c,
                };
                if super_c != next_c {
                    result.push(super_c);
                    i += 2;
                } else {
                    result.push('^');
                    i += 1;
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }
        result
    }

    /// Transpile simple subscripts like _0 to ₀ (only single standalone characters)
    fn replace_subscripts(text: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '_' && i + 1 < chars.len() {
                let next_c = chars[i + 1];
                // Only convert if the char after the candidate is NOT a letter
                let after_is_letter = i + 2 < chars.len() && chars[i + 2].is_ascii_alphabetic();
                let is_letter_candidate = next_c.is_ascii_alphabetic();
                if is_letter_candidate && after_is_letter {
                    // Part of a multi-char name like _{max} → _max, keep as-is
                    result.push('_');
                    i += 1;
                    continue;
                }
                let sub_c = match next_c {
                    '0' => '₀', '1' => '₁', '2' => '₂', '3' => '₃', '4' => '₄',
                    '5' => '₅', '6' => '₆', '7' => '₇', '8' => '₈', '9' => '₉',
                    '+' => '₊', '-' => '₋', '=' => '₌', '(' => '₍', ')' => '₎',
                    'a' => 'ₐ', 'e' => 'ₑ', 'h' => 'ₕ', 'i' => 'ᵢ', 'j' => 'ⱼ',
                    'k' => 'ₖ', 'l' => 'ₗ', 'm' => 'ₘ', 'n' => 'ₙ', 'o' => 'ₒ',
                    'p' => 'ₚ', 'r' => 'ᵣ', 's' => 'ₛ', 't' => 'ₜ', 'u' => 'ᵤ',
                    'v' => 'ᵥ', 'x' => 'ₓ',
                    _ => next_c,
                };
                if sub_c != next_c {
                    result.push(sub_c);
                    i += 2;
                } else {
                    result.push('_');
                    i += 1;
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }
        result
    }

    /// Strip LaTeX formatting wrappers like \\mathbf{...} or \\text{...}
    fn strip_latex_wrappers(text: &str) -> String {
        let wrappers = [
            "\\mathbf{",
            "\\text{",
            "\\mathrm{",
            "\\mathit{",
            "\\mathsf{",
            "\\mathtt{",
            "\\boldsymbol{",
            "\\textbf{",
            "\\textit{",
            "\\textrm{",
            "\\operatorname{",
            "\\bar{",
            "\\vec{",
            "\\hat{",
            "\\tilde{",
            "\\overline{",
            "\\underline{",
            "\\overbrace{",
            "\\underbrace{",
            "\\boxed{",
        ];

        let mut result = text.to_string();
        let mut changed = true;

        while changed {
            changed = false;
            for wrapper in &wrappers {
                if let Some(start_idx) = result.find(wrapper) {
                    let content_start = start_idx + wrapper.len();
                    let mut depth = 1;
                    let mut end_idx = None;
                    let chars: Vec<char> = result[content_start..].chars().collect();
                    let mut char_byte_offset = 0;
                    for c in chars {
                        if c == '{' {
                            depth += 1;
                        } else if c == '}' {
                            depth -= 1;
                            if depth == 0 {
                                end_idx = Some(content_start + char_byte_offset);
                                break;
                            }
                        }
                        char_byte_offset += c.len_utf8();
                    }

                    if let Some(end) = end_idx {
                        let prefix = &result[..start_idx];
                        let content = &result[content_start..end];
                        let suffix = &result[end + 1..];
                        result = format!("{}{}{}", prefix, content, suffix);
                        changed = true;
                        break;
                    }
                }
            }
        }
        result
    }

    /// Transpile \\mathbb{X} to blackboard bold Unicode (ℕ, ℤ, ℝ, ℚ, ℂ, etc.)
    fn strip_latex_mathbb(text: &str) -> String {
        let mut result = text.to_string();
        let mut changed = true;

        let bb_map: &[(&str, &str)] = &[
            ("A", "𝔸"), ("B", "𝔹"), ("C", "ℂ"), ("D", "𝔻"),
            ("E", "𝔼"), ("F", "𝔽"), ("G", "𝔾"), ("H", "ℍ"),
            ("I", "𝕀"), ("J", "𝕁"), ("K", "𝕂"), ("L", "𝕃"),
            ("M", "𝕄"), ("N", "ℕ"), ("O", "𝕆"), ("P", "ℙ"),
            ("Q", "ℚ"), ("R", "ℝ"), ("S", "𝕊"), ("T", "𝕋"),
            ("U", "𝕌"), ("V", "𝕍"), ("W", "𝕎"), ("X", "𝕏"),
            ("Y", "𝕐"), ("Z", "ℤ"),
        ];

        while changed {
            changed = false;
            if let Some(start_idx) = result.find("\\mathbb{") {
                let content_start = start_idx + 8;
                let mut depth = 1;
                let mut end_idx = None;
                let chars: Vec<char> = result[content_start..].chars().collect();
                let mut char_byte_offset = 0;
                for c in chars {
                    if c == '{' {
                        depth += 1;
                    } else if c == '}' {
                        depth -= 1;
                        if depth == 0 {
                            end_idx = Some(content_start + char_byte_offset);
                            break;
                        }
                    }
                    char_byte_offset += c.len_utf8();
                }

                if let Some(end) = end_idx {
                    let prefix = &result[..start_idx];
                    let content = &result[content_start..end];
                    let suffix = &result[end + 1..];
                    // Map each character to blackboard bold if possible
                    let mut mapped = String::new();
                    for ch in content.chars() {
                        let s = ch.to_string();
                        if let Some((_, bb)) = bb_map.iter().find(|(k, _)| *k == s) {
                            mapped.push_str(bb);
                        } else {
                            mapped.push(ch);
                        }
                    }
                    result = format!("{}{}{}", prefix, mapped, suffix);
                    changed = true;
                }
            }
        }
        result
    }

    /// Transpile \\pmod{content} to (mod content) and \\bmod to mod
    fn strip_latex_pmod(text: &str) -> String {
        let mut result = text.to_string();
        let mut changed = true;

        while changed {
            changed = false;
            if let Some(start_idx) = result.find("\\pmod{") {
                let content_start = start_idx + 6;
                let mut depth = 1;
                let mut end_idx = None;
                let chars: Vec<char> = result[content_start..].chars().collect();
                let mut char_byte_offset = 0;
                for c in chars {
                    if c == '{' {
                        depth += 1;
                    } else if c == '}' {
                        depth -= 1;
                        if depth == 0 {
                            end_idx = Some(content_start + char_byte_offset);
                            break;
                        }
                    }
                    char_byte_offset += c.len_utf8();
                }

                if let Some(end) = end_idx {
                    let prefix = &result[..start_idx];
                    let content = &result[content_start..end];
                    let suffix = &result[end + 1..];
                    result = format!("{} (mod {}){}", prefix, content, suffix);
                    changed = true;
                }
            }
        }
        // Also handle \bmod (binary mod operator)
        result = result.replace("\\bmod", "mod");
        result
    }

    /// Strip \\begin{...} and \\end{...} LaTeX environment markers
    fn strip_latex_environments(text: &str) -> String {
        let mut result = text.to_string();
        let mut changed = true;

        while changed {
            changed = false;
            for marker in &["\\begin{", "\\end{"] {
                if let Some(start_idx) = result.find(marker) {
                    let content_start = start_idx + marker.len();
                    if let Some(close) = result[content_start..].find('}') {
                        let end_idx = content_start + close;
                        let prefix = &result[..start_idx];
                        let suffix = &result[end_idx + 1..];
                        result = format!("{}{}", prefix, suffix);
                        changed = true;
                        break;
                    }
                }
            }
        }
        result
    }

    /// Transpile \\frac{numerator}{denominator} into (numerator)/(denominator)
    fn strip_latex_fractions(text: &str) -> String {
        let mut result = text.to_string();
        let mut changed = true;

        while changed {
            changed = false;
            if let Some(start_idx) = result.find("\\frac{") {
                let num_start = start_idx + 6;
                let mut depth = 1;
                let mut num_end = None;
                let chars: Vec<char> = result[num_start..].chars().collect();
                let mut char_byte_offset = 0;
                for c in chars {
                    if c == '{' {
                        depth += 1;
                    } else if c == '}' {
                        depth -= 1;
                        if depth == 0 {
                            num_end = Some(num_start + char_byte_offset);
                            break;
                        }
                    }
                    char_byte_offset += c.len_utf8();
                }

                if let Some(n_end) = num_end {
                    let rest = &result[n_end + 1..];
                    if rest.starts_with('{') {
                        let den_start = n_end + 2;
                        let mut depth = 1;
                        let mut den_end = None;
                        let chars: Vec<char> = result[den_start..].chars().collect();
                        let mut char_byte_offset = 0;
                        for c in chars {
                            if c == '{' {
                                depth += 1;
                            } else if c == '}' {
                                depth -= 1;
                                if depth == 0 {
                                    den_end = Some(den_start + char_byte_offset);
                                    break;
                                }
                            }
                            char_byte_offset += c.len_utf8();
                        }

                        if let Some(d_end) = den_end {
                            let prefix = &result[..start_idx];
                            let numerator = &result[num_start..n_end];
                            let denominator = &result[den_start..d_end];
                            let suffix = &result[d_end + 1..];
                            result = format!("{}({})/({}){}", prefix, numerator, denominator, suffix);
                            changed = true;
                            continue;
                        }
                    }
                }
            }
        }
        result
    }

    /// Transpile \\sqrt{content} to √(content)
    fn transpile_sqrt(text: &str) -> String {
        let mut result = text.to_string();
        let mut changed = true;

        while changed {
            changed = false;
            if let Some(start_idx) = result.find("\\sqrt{") {
                let content_start = start_idx + 6;
                let mut depth = 1;
                let mut end_idx = None;
                let chars: Vec<char> = result[content_start..].chars().collect();
                let mut char_byte_offset = 0;
                for c in chars {
                    if c == '{' {
                        depth += 1;
                    } else if c == '}' {
                        depth -= 1;
                        if depth == 0 {
                            end_idx = Some(content_start + char_byte_offset);
                            break;
                        }
                    }
                    char_byte_offset += c.len_utf8();
                }

                if let Some(end) = end_idx {
                    let prefix = &result[..start_idx];
                    let content = &result[content_start..end];
                    let suffix = &result[end + 1..];
                    result = format!("{}√({}){}", prefix, content, suffix);
                    changed = true;
                }
            }
        }
        result
    }

    /// Strip curly braces from subscripts/superscripts, e.g., _{max} -> _max, ^{2} -> ^2
    fn strip_sub_super_braces(text: &str) -> String {
        let mut result = text.to_string();
        let mut changed = true;

        while changed {
            changed = false;
            for prefix in &["_{", "^{"] {
                if let Some(start_idx) = result.find(prefix) {
                    let content_start = start_idx + 2;
                    let mut depth = 1;
                    let mut end_idx = None;
                    let chars: Vec<char> = result[content_start..].chars().collect();
                    let mut char_byte_offset = 0;
                    for c in chars {
                        if c == '{' {
                            depth += 1;
                        } else if c == '}' {
                            depth -= 1;
                            if depth == 0 {
                                end_idx = Some(content_start + char_byte_offset);
                                break;
                            }
                        }
                        char_byte_offset += c.len_utf8();
                    }

                    if let Some(end) = end_idx {
                        let pre = &result[..start_idx];
                        let content = &result[content_start..end];
                        let suffix = &result[end + 1..];
                        let symbol = &prefix[..1];
                        result = format!("{}{}{}{}", pre, symbol, content, suffix);
                        changed = true;
                        break;
                    }
                }
            }
        }
        result
    }

    /// Transpile common LaTeX macros into high-fidelity mathematical Unicode symbols
    pub fn transpile_latex_to_unicode(math: &str) -> String {
        let mut result = math.to_string();

        // Handle structural LaTeX directives first (brace-based)
        result = Self::transpile_sqrt(&result);
        result = Self::strip_latex_fractions(&result);
        result = Self::strip_latex_mathbb(&result);
        result = Self::strip_latex_pmod(&result);
        result = Self::strip_latex_environments(&result);
        result = Self::strip_latex_wrappers(&result);
        result = Self::strip_sub_super_braces(&result);

        // IMPORTANT: Substitutions are ordered longest-first to prevent
        // shorter patterns from greedily matching inside longer ones.
        // e.g. \partial must come before \par, \implies before \in, etc.
        let substitutions = [
            // Multi-character operators (longest first to avoid collisions)
            ("\\rightarrow", "→"),
            ("\\leftarrow", "←"),
            ("\\Rightarrow", "⇒"),
            ("\\Leftarrow", "⇐"),
            ("\\Leftrightarrow", "⇔"),
            ("\\leftrightarrow", "↔"),
            ("\\longrightarrow", "⟶"),
            ("\\longleftarrow", "⟵"),
            ("\\implies", "⇒"),
            ("\\impliedby", "⇐"),
            ("\\mapsto", "↦"),
            ("\\boldsymbol", ""),
            ("\\partial", "∂"),
            ("\\epsilon", "ε"),
            ("\\varepsilon", "ε"),
            ("\\upsilon", "υ"),
            ("\\varphi", "φ"),
            ("\\approx", "≈"),
            ("\\propto", "∝"),
            ("\\langle", "⟨"),
            ("\\rangle", "⟩"),
            ("\\lfloor", "⌊"),
            ("\\rfloor", "⌋"),
            ("\\lceil", "⌈"),
            ("\\rceil", "⌉"),
            ("\\subset", "⊂"),
            ("\\supset", "⊃"),
            ("\\subseteq", "⊆"),
            ("\\supseteq", "⊇"),
            ("\\emptyset", "∅"),
            ("\\notin", "∉"),
            ("\\nabla", "∇"),
            ("\\forall", "∀"),
            ("\\exists", "∃"),
            ("\\nexists", "∄"),
            ("\\lambda", "λ"),
            ("\\Lambda", "Λ"),
            ("\\vartheta", "ϑ"),
            ("\\varrho", "ϱ"),
            ("\\varsigma", "ς"),
            // Dots (before shorter matches)
            ("\\cdots", "⋯"),
            ("\\ldots", "…"),
            ("\\vdots", "⋮"),
            ("\\ddots", "⋱"),
            ("\\dots", "…"),
            // Greek letters
            ("\\alpha", "α"),
            ("\\beta", "β"),
            ("\\gamma", "γ"),
            ("\\delta", "δ"),
            ("\\zeta", "ζ"),
            ("\\eta", "η"),
            ("\\theta", "θ"),
            ("\\iota", "ι"),
            ("\\kappa", "κ"),
            ("\\mu", "μ"),
            ("\\nu", "ν"),
            ("\\xi", "ξ"),
            ("\\pi", "π"),
            ("\\rho", "ρ"),
            ("\\sigma", "σ"),
            ("\\tau", "τ"),
            ("\\phi", "φ"),
            ("\\chi", "χ"),
            ("\\psi", "ψ"),
            ("\\omega", "ω"),
            // Uppercase Greek
            ("\\Delta", "Δ"),
            ("\\Gamma", "Γ"),
            ("\\Theta", "Θ"),
            ("\\Pi", "Π"),
            ("\\Sigma", "Σ"),
            ("\\Phi", "Φ"),
            ("\\Psi", "Ψ"),
            ("\\Omega", "Ω"),
            ("\\Xi", "Ξ"),
            // Operators and relations
            ("\\infty", "∞"),
            ("\\times", "×"),
            ("\\equiv", "≡"),
            ("\\cong", "≅"),
            ("\\simeq", "≃"),
            ("\\cdot", "·"),
            ("\\circ", "∘"),
            ("\\star", "⋆"),
            ("\\bullet", "•"),
            ("\\div", "÷"),
            ("\\leq", "≤"),
            ("\\geq", "≥"),
            ("\\neq", "≠"),
            ("\\pm", "±"),
            ("\\mp", "∓"),
            ("\\le", "≤"),
            ("\\ge", "≥"),
            ("\\ne", "≠"),
            ("\\ll", "≪"),
            ("\\gg", "≫"),
            ("\\iff", "⇔"),
            ("\\neg", "¬"),
            ("\\land", "∧"),
            ("\\lor", "∨"),
            ("\\oplus", "⊕"),
            ("\\otimes", "⊗"),
            // Big operators
            ("\\sum", "∑"),
            ("\\prod", "∏"),
            ("\\coprod", "∐"),
            ("\\int", "∫"),
            ("\\iint", "∬"),
            ("\\iiint", "∭"),
            ("\\oint", "∮"),
            ("\\sqrt", "√"),
            ("\\cup", "∪"),
            ("\\cap", "∩"),
            ("\\bigcup", "⋃"),
            ("\\bigcap", "⋂"),
            // Spacing and sizing (strip to clean whitespace)
            ("\\quad", "  "),
            ("\\qquad", "    "),
            ("\\,", " "),
            ("\\;", " "),
            ("\\:", " "),
            ("\\!", ""),
            ("\\left", ""),
            ("\\right", ""),
            ("\\big", ""),
            ("\\Big", ""),
            ("\\bigg", ""),
            ("\\Bigg", ""),
            // Operator names (strip backslash, keep name)
            ("\\log", "log"),
            ("\\ln", "ln"),
            ("\\exp", "exp"),
            ("\\sin", "sin"),
            ("\\cos", "cos"),
            ("\\tan", "tan"),
            ("\\cot", "cot"),
            ("\\sec", "sec"),
            ("\\csc", "csc"),
            ("\\arcsin", "arcsin"),
            ("\\arccos", "arccos"),
            ("\\arctan", "arctan"),
            ("\\sinh", "sinh"),
            ("\\cosh", "cosh"),
            ("\\tanh", "tanh"),
            ("\\lim", "lim"),
            ("\\limsup", "lim sup"),
            ("\\liminf", "lim inf"),
            ("\\max", "max"),
            ("\\min", "min"),
            ("\\sup", "sup"),
            ("\\inf", "inf"),
            ("\\det", "det"),
            ("\\dim", "dim"),
            ("\\ker", "ker"),
            ("\\arg", "arg"),
            ("\\mod", "mod"),
            ("\\gcd", "gcd"),
            ("\\deg", "deg"),
            // Misc
            ("\\to", "→"),
            ("\\gets", "←"),
            ("\\in", "∈"),
            ("\\ni", "∋"),
            ("\\mid", "|"),
            ("\\parallel", "∥"),
            ("\\perp", "⊥"),
            ("\\angle", "∠"),
            ("\\triangle", "△"),
            ("\\prime", "′"),
            ("\\dagger", "†"),
            ("\\ddagger", "‡"),
            ("\\ell", "ℓ"),
            ("\\hbar", "ℏ"),
            ("\\Re", "ℜ"),
            ("\\Im", "ℑ"),
            ("\\wp", "℘"),
            ("\\aleph", "ℵ"),
        ];

        for (latex, unicode) in &substitutions {
            result = result.replace(latex, unicode);
        }

        result = Self::replace_superscripts(&result);
        result = Self::replace_subscripts(&result);
        result = result.replace("\\\\", "");
        result = result.replace("\\", "");
        result
    }

    /// Pre-transpile math segments in string before soft wrapping
    pub fn transpile_math_in_text(content: &str) -> String {
        let mut result = String::new();
        let mut remaining = content;

        while let Some(start_idx) = remaining.find('$') {
            result.push_str(&remaining[..start_idx]);
            let after_start = &remaining[start_idx + 1..];

            if after_start.starts_with('$') {
                // Block math $$...$$
                let after_double = &after_start[1..];
                if let Some(end_idx) = after_double.find("$$") {
                    let math_content = &after_double[..end_idx];
                    let transpiled = Self::transpile_latex_to_unicode(math_content);
                    result.push_str(&format!("$${}$$", transpiled));
                    remaining = &after_double[end_idx + 2..];
                } else {
                    result.push_str("$$");
                    remaining = after_double;
                }
            } else {
                // Inline math $...$
                if let Some(end_idx) = after_start.find('$') {
                    let math_content = &after_start[..end_idx];
                    let transpiled = Self::transpile_latex_to_unicode(math_content);
                    result.push_str(&format!("${}$", transpiled));
                    remaining = &after_start[end_idx + 1..];
                } else {
                    result.push_str("$");
                    remaining = after_start;
                }
            }
        }
        result.push_str(remaining);
        result
    }

    /// Split a transpiled line with $ markers into distinct normal and math-styled Spans
    pub fn parse_line_to_spans(text: &str, content_style: Style) -> Line<'static> {
        let mut spans = Vec::new();
        let mut remaining = text;

        let math_style = Style::default()
            .fg(Color::Rgb(129, 212, 250))
            .add_modifier(Modifier::ITALIC | Modifier::BOLD);

        while let Some(start_idx) = remaining.find('$') {
            let normal_part = &remaining[..start_idx];
            if !normal_part.is_empty() {
                spans.push(Span::styled(normal_part.to_string(), content_style));
            }

            let after_start = &remaining[start_idx + 1..];
            if after_start.starts_with('$') {
                let after_double = &after_start[1..];
                if let Some(end_idx) = after_double.find("$$") {
                    let math_content = &after_double[..end_idx];
                    spans.push(Span::styled(format!("  {}  ", math_content), math_style));
                    remaining = &after_double[end_idx + 2..];
                } else {
                    spans.push(Span::styled("$$", content_style));
                    remaining = after_double;
                }
            } else {
                if let Some(end_idx) = after_start.find('$') {
                    let math_content = &after_start[..end_idx];
                    spans.push(Span::styled(math_content.to_string(), math_style));
                    remaining = &after_start[end_idx + 1..];
                } else {
                    spans.push(Span::styled("$", content_style));
                    remaining = after_start;
                }
            }
        }

        if !remaining.is_empty() {
            spans.push(Span::styled(remaining.to_string(), content_style));
        }

        Line::from(spans)
    }

    /// Check if a line is a GFM table separator like `|---|---|`
    pub fn is_separator_line(line: &str) -> bool {
        let trimmed = line.trim();
        if !trimmed.contains('|') {
            return false;
        }
        let mut has_dash = false;
        for c in trimmed.chars() {
            if c == '-' {
                has_dash = true;
            } else if c != '|' && c != ':' && c != '+' && !c.is_whitespace() {
                return false;
            }
        }
        has_dash
    }

    /// Split a table row line by `|`, respecting escaped `\|`
    pub fn split_table_row(line: &str) -> Vec<String> {
        let mut cells = Vec::new();
        let mut current_cell = String::new();
        let mut chars = line.chars().peekable();
        
        let mut first = true;
        while let Some(c) = chars.next() {
            if c == '\\' && chars.peek() == Some(&'|') {
                current_cell.push('|');
                chars.next();
            } else if c == '|' {
                if first {
                    first = false;
                    let trimmed_before = line.trim_start();
                    if trimmed_before.starts_with('|') && current_cell.trim().is_empty() {
                        current_cell.clear();
                        continue;
                    }
                }
                cells.push(current_cell.trim().to_string());
                current_cell.clear();
            } else {
                current_cell.push(c);
            }
        }
        let last_trimmed = current_cell.trim();
        if !last_trimmed.is_empty() || !line.trim_end().ends_with('|') {
            cells.push(last_trimmed.to_string());
        }
        cells
    }

    /// Parse table column alignment from separator cell
    pub fn parse_alignment(cell: &str) -> TableAlign {
        let trimmed = cell.trim();
        let left = trimmed.starts_with(':');
        let right = trimmed.ends_with(':');
        if left && right {
            TableAlign::Center
        } else if right {
            TableAlign::Right
        } else {
            TableAlign::Left
        }
    }

    /// Parse markdown into text blocks and table blocks
    pub fn parse_markdown_blocks(content: &str) -> Vec<ContentBlock> {
        let mut blocks = Vec::new();
        let mut current_markdown = String::new();
        let mut in_code_block = false;
        
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();
            
            if trimmed.starts_with("```") {
                in_code_block = !in_code_block;
                current_markdown.push_str(line);
                current_markdown.push('\n');
                i += 1;
                continue;
            }
            
            if in_code_block {
                current_markdown.push_str(line);
                current_markdown.push('\n');
                i += 1;
                continue;
            }
            
            // Look ahead for table headers and separator rows
            let is_header_candidate = line.contains('|');
            let has_next = i + 1 < lines.len();
            let is_next_separator = has_next && Self::is_separator_line(lines[i + 1]);
            
            if is_header_candidate && is_next_separator {
                // Flush current markdown if not empty
                if !current_markdown.is_empty() {
                    blocks.push(ContentBlock::Markdown(current_markdown.clone()));
                    current_markdown.clear();
                }
                
                let header_line = line;
                let separator_line = lines[i + 1];
                
                let headers = Self::split_table_row(header_line);
                let sep_cells = Self::split_table_row(separator_line);
                
                let alignments: Vec<TableAlign> = sep_cells
                    .iter()
                    .map(|cell| Self::parse_alignment(cell))
                    .collect();
                
                let mut rows = Vec::new();
                i += 2; // skip header and separator
                
                while i < lines.len() {
                    let data_line = lines[i];
                    let trimmed_data = data_line.trim();
                    
                    if trimmed_data.starts_with("```") {
                        break;
                    }
                    
                    if data_line.contains('|') {
                        if Self::is_separator_line(data_line) {
                            break;
                        }
                        rows.push(Self::split_table_row(data_line));
                        i += 1;
                    } else {
                        break;
                    }
                }
                
                blocks.push(ContentBlock::Table {
                    headers,
                    alignments,
                    rows,
                });
                
                continue;
            }
            
            current_markdown.push_str(line);
            current_markdown.push('\n');
            i += 1;
        }
        
        if !current_markdown.is_empty() {
            blocks.push(ContentBlock::Markdown(current_markdown));
        }
        
        blocks
    }

    /// Truncate a Line's spans to a maximum width, adding an ellipsis if needed
    pub fn truncate_line(line: Line<'static>, max_w: usize) -> Line<'static> {
        let line_width = line.spans.iter().map(|s| s.content.width()).sum::<usize>();
        if line_width <= max_w {
            return line;
        }
        
        if max_w <= 1 {
            return Line::from(vec![Span::styled("…", Style::default().fg(Color::Rgb(120, 144, 156)))]);
        }
        
        let target_w = max_w - 1;
        let mut total_w = 0;
        let mut new_spans = Vec::new();
        let mut truncated = false;
        
        for span in line.spans {
            let span_w = span.content.width();
            if total_w + span_w <= target_w {
                new_spans.push(span.clone());
                total_w += span_w;
            } else {
                let mut prefix = String::new();
                for c in span.content.chars() {
                    let c_w = c.width().unwrap_or(0);
                    if total_w + c_w > target_w {
                        break;
                    }
                    prefix.push(c);
                    total_w += c_w;
                }
                if !prefix.is_empty() {
                    new_spans.push(Span::styled(prefix, span.style));
                }
                truncated = true;
                break;
            }
        }
        
        if truncated || total_w < line_width {
            new_spans.push(Span::styled("…", Style::default().fg(Color::Rgb(120, 144, 156))));
        }
        
        Line::from(new_spans)
    }

    /// Render a table row to styled TUI Lines, supporting multi-line wrapping and padding
    pub fn render_table_row(
        cells: &[String],
        col_widths: &[usize],
        alignments: &[TableAlign],
        border_style: Style,
        cell_style: Style,
    ) -> Vec<Line<'static>> {
        let col_count = col_widths.len();
        
        // Wrap and split cell contents for each column
        let mut cell_wrapped_lines: Vec<Vec<Line<'static>>> = Vec::new();
        for (i, cell_text) in cells.iter().enumerate() {
            let w = col_widths[i];
            
            // Replace <br> and <br/> with newline characters
            let clean_text = cell_text.replace("<br>", "\n").replace("<br/>", "\n");
            
            let mut col_lines = Vec::new();
            for part in clean_text.split('\n') {
                let parsed_line = Self::parse_line_to_spans(part, cell_style);
                let wrapped = Self::wrap_line(parsed_line, w);
                col_lines.extend(wrapped);
            }
            cell_wrapped_lines.push(col_lines);
        }
        
        // Determine max line count for this row
        let line_count = cell_wrapped_lines.iter().map(|lines| lines.len()).max().unwrap_or(1);
        
        let mut sub_rows = Vec::new();
        for sub_idx in 0..line_count {
            let mut row_spans = Vec::new();
            row_spans.push(Span::styled("│", border_style));
            
            for i in 0..col_count {
                let w = col_widths[i];
                let align = alignments[i];
                
                // Get the line for this sub-row, or default to an empty line if this cell has fewer lines
                let cell_line = if sub_idx < cell_wrapped_lines[i].len() {
                    cell_wrapped_lines[i][sub_idx].clone()
                } else {
                    Line::from(Vec::new())
                };
                
                let line_width = cell_line.spans.iter().map(|s| s.content.width()).sum::<usize>();
                let remaining_w = w.saturating_sub(line_width);
                
                let (left_pad, right_pad) = match align {
                    TableAlign::Left => (0, remaining_w),
                    TableAlign::Right => (remaining_w, 0),
                    TableAlign::Center => {
                        let lp = remaining_w / 2;
                        let rp = remaining_w - lp;
                        (lp, rp)
                    }
                };
                
                row_spans.push(Span::styled(" ", cell_style));
                if left_pad > 0 {
                    row_spans.push(Span::styled(" ".repeat(left_pad), cell_style));
                }
                for span in cell_line.spans {
                    row_spans.push(span);
                }
                if right_pad > 0 {
                    row_spans.push(Span::styled(" ".repeat(right_pad), cell_style));
                }
                row_spans.push(Span::styled(" ", cell_style));
                
                row_spans.push(Span::styled("│", border_style));
            }
            sub_rows.push(Line::from(row_spans));
        }
        
        sub_rows
    }

    /// Render a complete GFM table with Unicode borders and alignments
    pub fn render_table(
        headers: Vec<String>,
        alignments: Vec<TableAlign>,
        rows: Vec<Vec<String>>,
        viewport_width: usize,
        content_style: Style,
    ) -> Vec<Line<'static>> {
        let col_count = headers.len();
        if col_count == 0 {
            return Vec::new();
        }
        
        let mut alignments = alignments;
        while alignments.len() < col_count {
            alignments.push(TableAlign::Left);
        }
        
        let mut formatted_rows = Vec::new();
        for row in rows {
            let mut cells = row;
            while cells.len() < col_count {
                cells.push(String::new());
            }
            if cells.len() > col_count {
                cells.truncate(col_count);
            }
            formatted_rows.push(cells);
        }
        
        // Calculate max natural width of columns, clamping them reasonably
        let max_natural_width = (viewport_width / 3).max(20);
        let mut col_widths = vec![0; col_count];
        for (i, h) in headers.iter().enumerate() {
            let header_line = Self::parse_line_to_spans(h, content_style);
            let header_width = header_line.spans.iter().map(|s| s.content.width()).sum::<usize>();
            col_widths[i] = col_widths[i].max(header_width.min(max_natural_width));
        }
        for row in &formatted_rows {
            for (i, cell) in row.iter().enumerate() {
                let cell_line = Self::parse_line_to_spans(cell, content_style);
                let cell_width = cell_line.spans.iter().map(|s| s.content.width()).sum::<usize>();
                col_widths[i] = col_widths[i].max(cell_width.min(max_natural_width));
            }
        }
        
        // Check if table exceeds viewport width and scale down if needed
        let max_table_width = viewport_width.saturating_sub(4);
        let total_content_width: usize = col_widths.iter().sum();
        let total_table_width = 1 + total_content_width + 3 * col_count;
        
        if total_table_width > max_table_width && total_content_width > 0 {
            let available_content_width = max_table_width
                .saturating_sub(1)
                .saturating_sub(3 * col_count);
            
            if available_content_width >= col_count {
                let mut new_widths = vec![1; col_count];
                let mut assigned = col_count;
                let remaining_to_assign = available_content_width - col_count;
                
                if remaining_to_assign > 0 {
                    for i in 0..col_count {
                        let share = (col_widths[i] * remaining_to_assign) / total_content_width;
                        new_widths[i] += share;
                        assigned += share;
                    }
                    
                    let mut remainder = available_content_width - assigned;
                    while remainder > 0 {
                        let mut best_col = 0;
                        let mut max_diff = 0;
                        for i in 0..col_count {
                            if col_widths[i] > new_widths[i] {
                                let diff = col_widths[i] - new_widths[i];
                                if diff > max_diff {
                                    max_diff = diff;
                                    best_col = i;
                                }
                            }
                        }
                        new_widths[best_col] += 1;
                        remainder -= 1;
                    }
                }
                col_widths = new_widths;
            }
        }
        
        // Border styling and header styling
        let border_style = Style::default().fg(Color::Rgb(100, 116, 139));
        let header_style = content_style.add_modifier(Modifier::BOLD);
        
        let mut lines = Vec::new();
        
        // Render top border
        let mut top_spans = Vec::new();
        top_spans.push(Span::styled("┌", border_style));
        for (idx, &w) in col_widths.iter().enumerate() {
            top_spans.push(Span::styled("─".repeat(w + 2), border_style));
            if idx + 1 < col_widths.len() {
                top_spans.push(Span::styled("┬", border_style));
            }
        }
        top_spans.push(Span::styled("┐", border_style));
        lines.push(Line::from(top_spans));
        
        // Render headers
        lines.extend(Self::render_table_row(&headers, &col_widths, &alignments, border_style, header_style));
        
        // Render separator and data rows
        if !formatted_rows.is_empty() {
            let mut sep_spans = Vec::new();
            sep_spans.push(Span::styled("├", border_style));
            for (idx, &w) in col_widths.iter().enumerate() {
                sep_spans.push(Span::styled("─".repeat(w + 2), border_style));
                if idx + 1 < col_widths.len() {
                    sep_spans.push(Span::styled("┼", border_style));
                }
            }
            sep_spans.push(Span::styled("┤", border_style));
            lines.push(Line::from(sep_spans));
            
            for row in &formatted_rows {
                lines.extend(Self::render_table_row(row, &col_widths, &alignments, border_style, content_style));
            }
        }
        
        // Render bottom border
        let mut bottom_spans = Vec::new();
        bottom_spans.push(Span::styled("└", border_style));
        for (idx, &w) in col_widths.iter().enumerate() {
            bottom_spans.push(Span::styled("─".repeat(w + 2), border_style));
            if idx + 1 < col_widths.len() {
                bottom_spans.push(Span::styled("┴", border_style));
            }
        }
        bottom_spans.push(Span::styled("┘", border_style));
        lines.push(Line::from(bottom_spans));
        
        lines
    }

    /// Wrap a Line containing multiple Spans cleanly without breaking formulas
    pub fn wrap_line(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
        if width == 0 {
            return vec![line];
        }

        let mut wrapped_lines = Vec::new();
        let mut current_spans = Vec::new();
        let mut current_width = 0;

        for span in line.spans {
            let style = span.style;
            let text = span.content.to_string();
            let words = text.split_inclusive(' ');

            for word in words {
                use unicode_width::UnicodeWidthStr;
                let word_width = word.width();

                if current_width + word_width <= width {
                    current_spans.push(Span::styled(word.to_string(), style));
                    current_width += word_width;
                } else if word_width >= width {
                    if !current_spans.is_empty() {
                        wrapped_lines.push(Line::from(current_spans));
                        current_spans = Vec::new();
                        current_width = 0;
                    }
                    let mut current_word_chunk = String::new();
                    let mut chunk_width = 0;
                    for c in word.chars() {
                        let c_width = c.width().unwrap_or(0);
                        if chunk_width + c_width > width {
                            wrapped_lines.push(Line::from(Span::styled(current_word_chunk, style)));
                            current_word_chunk = String::new();
                            chunk_width = 0;
                        }
                        current_word_chunk.push(c);
                        chunk_width += c_width;
                    }
                    if !current_word_chunk.is_empty() {
                        current_spans.push(Span::styled(current_word_chunk, style));
                        current_width = chunk_width;
                    }
                } else {
                    wrapped_lines.push(Line::from(current_spans));
                    current_spans = vec![Span::styled(word.to_string(), style)];
                    current_width = word_width;
                }
            }
        }

        if !current_spans.is_empty() {
            wrapped_lines.push(Line::from(current_spans));
        }

        if wrapped_lines.is_empty() {
            wrapped_lines.push(Line::from(String::new()));
        }

        wrapped_lines
    }

    pub fn update_cache(&mut self, viewport_width: usize, show_reasoning: bool) {
        self.cached_visual_lines.clear();
        if viewport_width == 0 {
            return;
        }

        // Message header with role
        let (icon, header_style, content_style) = match self.role {
            MessageRole::User => (
                icons::USER,
                Style::default()
                    .fg(Color::Rgb(129, 199, 132))
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Rgb(224, 247, 250)),
            ),
            MessageRole::Assistant => (
                icons::ASSISTANT,
                Style::default()
                    .fg(Color::Rgb(144, 202, 249))
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Rgb(189, 189, 189)),
            ),
            MessageRole::System => (
                icons::SYSTEM,
                Style::default()
                    .fg(Color::Rgb(176, 190, 197))
                    .add_modifier(Modifier::ITALIC),
                Style::default().fg(Color::Rgb(158, 158, 158)),
            ),
        };

        // Add separator line
        self.cached_visual_lines.push(Line::from(Span::styled(
            format!(
                "━━━ {} {} ━━━",
                icon,
                match self.role {
                    MessageRole::User => "You",
                    MessageRole::Assistant => "Assistant",
                    MessageRole::System => "System",
                }
            ),
            header_style,
        )));

        // Parse inline thoughts if any
        let (inline_thought, display_content) = if self.role == MessageRole::Assistant {
            Self::parse_inline_thought(&self.content)
        } else {
            (None, self.content.clone())
        };

        let combined_thought = match (&self.reasoning, &inline_thought) {
            (Some(r), Some(i)) => Some(format!("{}\n{}", r, i)),
            (Some(r), None) => Some(r.clone()),
            (None, Some(i)) => Some(i.clone()),
            (None, None) => None,
        };

        // Render thought block first if enabled
        if show_reasoning {
            if let Some(ref thought) = combined_thought {
                if !thought.is_empty() {
                    self.cached_visual_lines.push(Line::from(Span::styled(
                        "  ⚡ Thought Process".to_string(),
                        Style::default()
                            .fg(Color::Rgb(255, 183, 77))
                            .add_modifier(Modifier::ITALIC | Modifier::BOLD),
                    )));
                    let reasoning_style = Style::default()
                        .fg(Color::Rgb(120, 144, 156))
                        .add_modifier(Modifier::ITALIC);
                    for line in thought.lines() {
                        let text = format!("    {}", line);
                        if text.width() <= viewport_width {
                            self.cached_visual_lines.push(Line::from(Span::styled(text, reasoning_style)));
                        } else {
                            let wrapped = ChatApp::wrap_text_to_width(&text, viewport_width);
                            for wrapped_line in wrapped {
                                self.cached_visual_lines.push(Line::from(Span::styled(wrapped_line, reasoning_style)));
                            }
                        }
                    }
                    self.cached_visual_lines.push(Line::from(String::new()));
                }
            }
        }

        // Pre-transpile math segments in the display content
        let display_content_transpiled = Self::transpile_math_in_text(&display_content);

        // Render message content into logical lines
        if self.role == MessageRole::Assistant {
            let blocks = Self::parse_markdown_blocks(&display_content_transpiled);
            for block in blocks {
                match block {
                    ContentBlock::Markdown(text) => {
                        let rendered = tui_markdown::from_str(&text);
                        for line in rendered.lines {
                            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                            let parsed_line = Self::parse_line_to_spans(&text, content_style);
                            let wrapped = Self::wrap_line(parsed_line, viewport_width);
                            self.cached_visual_lines.extend(wrapped);
                        }
                    }
                    ContentBlock::Table { headers, alignments, rows } => {
                        let table_lines = Self::render_table(headers, alignments, rows, viewport_width, content_style);
                        self.cached_visual_lines.extend(table_lines);
                    }
                }
            }
        } else {
            let mut logical_lines = Vec::new();
            for line in display_content_transpiled.lines() {
                logical_lines.push((format!("  {}", line), content_style));
            }
            for (text, style) in logical_lines {
                let parsed_line = Self::parse_line_to_spans(&text, style);
                let wrapped = Self::wrap_line(parsed_line, viewport_width);
                self.cached_visual_lines.extend(wrapped);
            }
        }

        // Add spacing at the end
        self.cached_visual_lines.push(Line::from(String::new()));
    }
}

/// Elegant Chat application state
pub struct ChatApp {
    /// Chat message history
    messages: Vec<ChatMessage>,
    /// Simple input widget
    input: SimpleInput,
    /// Scroll offset for message display
    scroll_offset: usize,
    /// Buffer for streaming response
    streaming_buffer: String,
    /// Buffer for streaming reasoning response
    streaming_reasoning: String,
    /// Toggle to show/hide reasoning tokens
    show_reasoning: bool,
    /// Whether currently streaming a response
    is_streaming: bool,
    /// LLM provider
    provider: Arc<GenAIProvider>,
    /// Model to use
    model: String,
    /// Throbber state for loading animation
    throbber_state: ThrobberState,
    /// Should quit the application
    should_quit: bool,
    /// Receiver for streaming chunks
    stream_rx: Option<mpsc::UnboundedReceiver<String>>,
    /// Total visual lines in messages (for scrolling) - after wrapping
    total_visual_lines: usize,
    /// Auto-scroll to bottom flag (set during streaming)
    auto_scroll: bool,
    /// Visible height of message area (updated during render)
    visible_height: usize,
    /// Flag to indicate a message send is pending (for async handling)
    pending_send: bool,
    /// Last viewport width used for wrapping (to detect resize)
    last_viewport_width: usize,
    /// Shared history loaded from data_dir/history.txt
    history: Vec<String>,
    /// Index of the current traversed history item
    history_index: Option<usize>,
    /// Current input draft when traversing history
    history_draft: String,
}

impl ChatApp {
    /// Save the current conversation history to a file in markdown format
    pub fn save_conversation_to_file(&self, filepath: &str) -> std::io::Result<()> {
        use std::io::Write;
        
        let path = std::path::Path::new(filepath);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let mut file = std::fs::File::create(path)?;
        writeln!(file, "# Perspt Conversation Session")?;
        writeln!(file, "Model: {}\n", self.model)?;
        
        for msg in &self.messages {
            match msg.role {
                MessageRole::User => {
                    writeln!(file, "## You\n{}\n", msg.content)?;
                }
                MessageRole::Assistant => {
                    writeln!(file, "## Assistant\n")?;
                    if self.show_reasoning {
                        if let Some(ref thought) = msg.reasoning {
                            if !thought.is_empty() {
                                writeln!(file, "> [!NOTE]")?;
                                writeln!(file, "> **Thought Process**")?;
                                for line in thought.lines() {
                                    writeln!(file, "> {}", line)?;
                                }
                                writeln!(file, "\n")?;
                            }
                        }
                    }
                    writeln!(file, "{}\n", msg.content)?;
                }
                MessageRole::System => {
                    writeln!(file, "*System: {}\n*", msg.content)?;
                }
            }
        }
        
        Ok(())
    }

    /// Create a new chat application
    pub fn new(provider: GenAIProvider, model: String) -> Self {
        // Load history from paths::history_file() if possible
        let mut history = Vec::new();
        if let Some(history_path) = perspt_core::paths::history_file() {
            if history_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&history_path) {
                    history = content.lines().map(|s| s.to_string()).collect();
                }
            }
        }

        Self {
            messages: vec![ChatMessage::system(
                "Welcome to Perspt! Type your message and press Enter to send.",
            )],
            input: SimpleInput::new(),
            scroll_offset: 0,
            streaming_buffer: String::new(),
            streaming_reasoning: String::new(),
            show_reasoning: true,
            is_streaming: false,
            provider: Arc::new(provider),
            model,
            throbber_state: ThrobberState::default(),
            should_quit: false,
            stream_rx: None,
            total_visual_lines: 0,
            auto_scroll: true, // Start with auto-scroll enabled
            visible_height: 20,
            pending_send: false,
            last_viewport_width: 80,
            history,
            history_index: None,
            history_draft: String::new(),
        }
    }

    /// Run the chat application main loop
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            // Render
            terminal.draw(|frame| self.render(frame))?;

            // Handle streaming updates - drain ALL pending chunks before rendering
            let mut just_finalized = false;
            if let Some(ref mut rx) = self.stream_rx {
                loop {
                    match rx.try_recv() {
                        Ok(chunk) => {
                            if chunk == EOT_SIGNAL {
                                self.finalize_streaming();
                                just_finalized = true;
                                break;
                            } else if chunk.starts_with("__PERSPT_REASONING__:") {
                                let content = &chunk["__PERSPT_REASONING__:".len()..];
                                self.streaming_reasoning.push_str(content);
                            } else {
                                self.streaming_buffer.push_str(&chunk);
                            }
                        }
                        Err(mpsc::error::TryRecvError::Empty) => break,
                        Err(mpsc::error::TryRecvError::Disconnected) => {
                            self.finalize_streaming();
                            just_finalized = true;
                            break;
                        }
                    }
                }
            }

            // Immediate re-render after finalization to show final content without delay
            if just_finalized {
                terminal.draw(|frame| self.render(frame))?;
            }

            // Event handling
            let timeout = if self.is_streaming {
                std::time::Duration::from_millis(16) // ~60fps for smooth streaming
            } else {
                std::time::Duration::from_millis(100)
            };

            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.kind == KeyEventKind::Release {
                            continue;
                        }

                        match key.code {
                            // Quit
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.should_quit = true;
                            }
                            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.should_quit = true;
                            }
                            // Emacs navigation & editing shortcuts
                            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.move_home();
                            }
                            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.move_end();
                            }
                            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.move_left();
                            }
                            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.move_right();
                            }
                            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.delete();
                            }
                            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.backspace();
                            }
                            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.kill_to_end();
                            }
                            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.kill_to_start();
                            }
                            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.input.delete_word_before();
                            }

                            // Send message on Enter
                            KeyCode::Enter if !self.is_streaming && !self.input.is_empty() => {
                                let text = self.input.text().trim().to_string();
                                if text.starts_with('/') {
                                    let cmd = text.to_lowercase();
                                    if cmd == "/exit" || cmd == "/quit" {
                                        self.should_quit = true;
                                    } else if cmd == "/clear" {
                                        self.messages.clear();
                                        self.push_message(ChatMessage::system(
                                            "Conversation history cleared.",
                                        ));
                                        self.input.clear();
                                        self.scroll_offset = 0;
                                        let _ = terminal.clear(); // Explicitly clear terminal screen buffer to remove residual artifacts
                                    } else if cmd.starts_with("/model") {
                                        let parts: Vec<&str> = text.split_whitespace().collect();
                                        if parts.len() > 1 {
                                            let new_model = parts[1..].join(" ");
                                            self.model = new_model;
                                            self.push_message(ChatMessage::system(
                                                format!("Switched model to: {}", self.model),
                                            ));
                                        } else {
                                            self.push_message(ChatMessage::system(
                                                "Usage: /model <name>",
                                            ));
                                        }
                                        self.input.clear();
                                    } else if cmd.starts_with("/save") {
                                        let parts: Vec<&str> = text.split_whitespace().collect();
                                        if parts.len() > 1 {
                                            let filepath = parts[1..].join(" ");
                                            match self.save_conversation_to_file(&filepath) {
                                                Ok(_) => {
                                                    self.push_message(ChatMessage::system(
                                                        format!("Conversation saved successfully to: {}", filepath),
                                                    ));
                                                }
                                                Err(e) => {
                                                    self.push_message(ChatMessage::system(
                                                        format!("Failed to save conversation: {}", e),
                                                    ));
                                                }
                                            }
                                        } else {
                                            self.push_message(ChatMessage::system(
                                                "Usage: /save <file_path>\nExample: /save conversation.md",
                                            ));
                                        }
                                        self.input.clear();
                                    } else if cmd == "/help" {
                                        self.push_message(ChatMessage::system(
                                            "Available Slash Commands:\n  /exit, /quit      - Exit the chat session\n  /clear            - Reset the conversation history\n  /model <name>     - Switch the active model on the fly\n  /save <path>      - Export conversation history to a file\n  /help             - Show this help menu",
                                        ));
                                        self.input.clear();
                                    } else {
                                        self.push_message(ChatMessage::system(
                                            format!("Unknown command: {}. Type /help for help.", text),
                                        ));
                                        self.input.clear();
                                    }
                                } else {
                                    self.send_message().await?;
                                }
                            }
                            // Toggle reasoning display on Ctrl+R
                            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.show_reasoning = !self.show_reasoning;
                                for msg in &mut self.messages {
                                    msg.update_cache(self.last_viewport_width, self.show_reasoning);
                                }
                            }
                            // Newline with Ctrl+J (reliable across terminals)
                            KeyCode::Char('j')
                                if key.modifiers.contains(KeyModifiers::CONTROL)
                                    && !self.is_streaming =>
                            {
                                self.input.insert_newline();
                            }
                            // Also support Ctrl+Enter for newline
                            KeyCode::Enter
                                if key.modifiers.contains(KeyModifiers::CONTROL)
                                    && !self.is_streaming =>
                            {
                                self.input.insert_newline();
                            }
                            // Scroll
                            KeyCode::PageUp => self.scroll_up(10),
                            KeyCode::PageDown => self.scroll_down(10),
                            KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.scroll_up(1)
                            }
                            KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.scroll_down(1)
                            }
                            // Shift+Up/Down to scroll
                            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                self.scroll_up(1);
                            }
                            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                self.scroll_down(1);
                            }
                            // Input navigation / history traversal
                            KeyCode::Left => self.input.move_left(),
                            KeyCode::Right => self.input.move_right(),
                            KeyCode::Up if !self.is_streaming => {
                                if self.input.cursor_line() == 0 {
                                    if !self.history.is_empty() {
                                        if self.history_index.is_none() {
                                            self.history_draft = self.input.text();
                                            self.history_index = Some(self.history.len() - 1);
                                            let item = &self.history[self.history.len() - 1];
                                            self.input.set_text(item);
                                        } else {
                                            let idx = self.history_index.unwrap();
                                            if idx > 0 {
                                                self.history_index = Some(idx - 1);
                                                let item = &self.history[idx - 1];
                                                self.input.set_text(item);
                                            }
                                        }
                                    }
                                } else {
                                    self.input.move_up();
                                }
                            }
                            KeyCode::Down if !self.is_streaming => {
                                if self.input.cursor_line() == self.input.line_count() - 1 {
                                    if let Some(idx) = self.history_index {
                                        if idx + 1 < self.history.len() {
                                            self.history_index = Some(idx + 1);
                                            let item = &self.history[idx + 1];
                                            self.input.set_text(item);
                                        } else {
                                            self.history_index = None;
                                            let draft = self.history_draft.clone();
                                            self.input.set_text(&draft);
                                        }
                                    }
                                } else {
                                    self.input.move_down();
                                }
                            }
                            KeyCode::Home => self.input.move_home(),
                            KeyCode::End => self.input.move_end(),
                            // Text editing
                            KeyCode::Backspace => self.input.backspace(),
                            KeyCode::Delete => self.input.delete(),
                            KeyCode::Char(c) if !self.is_streaming => {
                                self.input.insert_char(c);
                            }
                            _ => {}
                        }
                    }
                    Event::Mouse(mouse) => match mouse.kind {
                        MouseEventKind::ScrollUp => self.scroll_up(3),
                        MouseEventKind::ScrollDown => self.scroll_down(3),
                        _ => {}
                    },
                    _ => {}
                }
            }

            // Update throbber
            if self.is_streaming {
                self.throbber_state.calc_next();
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Handle an AppEvent from the async event loop
    ///
    /// Returns `true` to continue running, `false` to quit.
    pub fn handle_app_event(&mut self, event: AppEvent) -> bool {
        match event {
            AppEvent::Terminal(crossterm_event) => self.handle_terminal_event(crossterm_event),
            AppEvent::StreamChunk(chunk) => {
                self.streaming_buffer.push_str(&chunk);
                true
            }
            AppEvent::StreamComplete => {
                self.finalize_streaming();
                true
            }
            AppEvent::Tick => {
                if self.is_streaming {
                    self.throbber_state.calc_next();
                }
                true
            }
            AppEvent::Quit => false,
            AppEvent::Error(e) => {
                // Log error but continue
                log::error!("App error: {}", e);
                true
            }
            AppEvent::AgentUpdate(_) => true, // Not used in chat mode
            AppEvent::CoreEvent(_) => true,   // Not used in chat mode
        }
    }

    /// Handle a terminal event (key press, mouse, resize)
    fn handle_terminal_event(&mut self, event: CrosstermEvent) -> bool {
        match event {
            CrosstermEvent::Key(key) => {
                if key.kind == KeyEventKind::Release {
                    return true;
                }

                match key.code {
                    // Quit
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return false;
                    }
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return false;
                    }
                    // Emacs navigation & editing shortcuts
                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.move_home();
                    }
                    KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.move_end();
                    }
                    KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.move_left();
                    }
                    KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.move_right();
                    }
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.delete();
                    }
                    KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.backspace();
                    }
                    KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.kill_to_end();
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.kill_to_start();
                    }
                    KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.delete_word_before();
                    }

                    // Send message on Enter
                    KeyCode::Enter if !self.is_streaming && !self.input.is_empty() => {
                        let text = self.input.text().trim().to_string();
                        if text.starts_with('/') {
                            let cmd = text.to_lowercase();
                            if cmd == "/exit" || cmd == "/quit" {
                                return false; // Exit TUI app
                            } else if cmd == "/clear" {
                                self.messages.clear();
                                self.push_message(ChatMessage::system(
                                    "Conversation history cleared.",
                                ));
                                self.input.clear();
                                self.scroll_offset = 0;
                            } else if cmd.starts_with("/model") {
                                let parts: Vec<&str> = text.split_whitespace().collect();
                                if parts.len() > 1 {
                                    let new_model = parts[1..].join(" ");
                                    self.model = new_model;
                                    self.push_message(ChatMessage::system(
                                        format!("Switched model to: {}", self.model),
                                    ));
                                } else {
                                    self.push_message(ChatMessage::system(
                                        "Usage: /model <name>",
                                    ));
                                }
                                self.input.clear();
                            } else if cmd.starts_with("/save") {
                                let parts: Vec<&str> = text.split_whitespace().collect();
                                if parts.len() > 1 {
                                    let filepath = parts[1..].join(" ");
                                    match self.save_conversation_to_file(&filepath) {
                                        Ok(_) => {
                                            self.push_message(ChatMessage::system(
                                                format!("Conversation saved successfully to: {}", filepath),
                                            ));
                                        }
                                        Err(e) => {
                                            self.push_message(ChatMessage::system(
                                                format!("Failed to save conversation: {}", e),
                                            ));
                                        }
                                    }
                                } else {
                                    self.push_message(ChatMessage::system(
                                        "Usage: /save <file_path>\nExample: /save conversation.md",
                                    ));
                                }
                                self.input.clear();
                            } else if cmd == "/help" {
                                self.push_message(ChatMessage::system(
                                    "Available Slash Commands:\n  /exit, /quit      - Exit the chat session\n  /clear            - Reset the conversation history\n  /model <name>     - Switch the active model on the fly\n  /save <path>      - Export conversation history to a file\n  /help             - Show this help menu",
                                ));
                                self.input.clear();
                            } else {
                                self.push_message(ChatMessage::system(
                                    format!("Unknown command: {}. Type /help for help.", text),
                                ));
                                self.input.clear();
                            }
                        } else {
                            self.pending_send = true;
                        }
                    }
                    // Toggle reasoning display on Ctrl+R
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.show_reasoning = !self.show_reasoning;
                        for msg in &mut self.messages {
                            msg.update_cache(self.last_viewport_width, self.show_reasoning);
                        }
                    }
                    // Newline with Ctrl+J
                    KeyCode::Char('j')
                        if key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming =>
                    {
                        self.input.insert_newline();
                    }
                    // Ctrl+Enter for newline
                    KeyCode::Enter
                        if key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming =>
                    {
                        self.input.insert_newline();
                    }
                    // Scroll
                    KeyCode::PageUp => self.scroll_up(10),
                    KeyCode::PageDown => self.scroll_down(10),
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.scroll_up(1)
                    }
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.scroll_down(1)
                    }
                    // Shift+Up/Down to scroll
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        self.scroll_up(1);
                    }
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        self.scroll_down(1);
                    }
                    // Input navigation / history traversal
                    KeyCode::Left => self.input.move_left(),
                    KeyCode::Right => self.input.move_right(),
                    KeyCode::Up if !self.is_streaming => {
                        if self.input.cursor_line() == 0 {
                            if !self.history.is_empty() {
                                if self.history_index.is_none() {
                                    self.history_draft = self.input.text();
                                    self.history_index = Some(self.history.len() - 1);
                                    let item = &self.history[self.history.len() - 1];
                                    self.input.set_text(item);
                                } else {
                                    let idx = self.history_index.unwrap();
                                    if idx > 0 {
                                        self.history_index = Some(idx - 1);
                                        let item = &self.history[idx - 1];
                                        self.input.set_text(item);
                                    }
                                }
                            }
                        } else {
                            self.input.move_up();
                        }
                    }
                    KeyCode::Down if !self.is_streaming => {
                        if self.input.cursor_line() == self.input.line_count() - 1 {
                            if let Some(idx) = self.history_index {
                                if idx + 1 < self.history.len() {
                                    self.history_index = Some(idx + 1);
                                    let item = &self.history[idx + 1];
                                    self.input.set_text(item);
                                } else {
                                    self.history_index = None;
                                    let draft = self.history_draft.clone();
                                    self.input.set_text(&draft);
                                }
                            }
                        } else {
                            self.input.move_down();
                        }
                    }
                    KeyCode::Home => self.input.move_home(),
                    KeyCode::End => self.input.move_end(),
                    // Text editing
                    KeyCode::Backspace => self.input.backspace(),
                    KeyCode::Delete => self.input.delete(),
                    KeyCode::Char(c) if !self.is_streaming => {
                        self.input.insert_char(c);
                    }
                    _ => {}
                }
            }
            CrosstermEvent::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => self.scroll_up(3),
                MouseEventKind::ScrollDown => self.scroll_down(3),
                _ => {}
            },
            CrosstermEvent::Resize(_, _) => {
                // Terminal resize - render will handle it
            }
            _ => {}
        }
        true
    }

    /// Check if a message send is pending (set by Enter key in handle_terminal_event)
    pub fn is_send_pending(&self) -> bool {
        self.pending_send
    }

    /// Clear the pending send flag
    pub fn clear_pending_send(&mut self) {
        self.pending_send = false;
    }

    /// Check and process pending stream chunks
    pub fn process_stream_chunks(&mut self) {
        if let Some(ref mut rx) = self.stream_rx {
            loop {
                match rx.try_recv() {
                    Ok(chunk) => {
                        if chunk == EOT_SIGNAL {
                            self.finalize_streaming();
                            break;
                        } else if chunk.starts_with("__PERSPT_REASONING__:") {
                            let content = &chunk["__PERSPT_REASONING__:".len()..];
                            self.streaming_reasoning.push_str(content);
                        } else {
                            self.streaming_buffer.push_str(&chunk);
                        }
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        self.finalize_streaming();
                        break;
                    }
                }
            }
        }
    }

    /// Check if a render is needed
    pub fn needs_render(&self) -> bool {
        self.is_streaming || self.pending_send
    }

    /// Prune messages if they exceed character count limits (32,000 chars)
    fn prune_messages(&mut self) {
        loop {
            let total_chars: usize = self.messages.iter().map(|m| m.content.len()).sum();
            if total_chars <= 32000 {
                break;
            }

            let remove_idx = if self.messages.first().map(|m| m.role == MessageRole::System).unwrap_or(false) {
                if self.messages.len() > 1 {
                    1
                } else {
                    break;
                }
            } else {
                0
            };

            if self.messages.len() > remove_idx {
                self.messages.remove(remove_idx);
            } else {
                break;
            }
        }
    }

    /// Add a message, updating its visual cache and pruning history automatically
    fn push_message(&mut self, mut msg: ChatMessage) {
        msg.update_cache(self.last_viewport_width, self.show_reasoning);
        self.messages.push(msg);
        self.prune_messages();
        self.scroll_to_bottom();
    }

    /// Send the current message to the LLM
    async fn send_message(&mut self) -> Result<()> {
        let user_message = self.input.text().trim().to_string();
        if user_message.is_empty() {
            return Ok(());
        }

        // Add user message
        let msg = ChatMessage::user(user_message.clone());
        self.push_message(msg);
        self.input.clear();

        // Save to history
        self.history.push(user_message.clone());
        self.history_index = None;
        self.history_draft.clear();
        if let Some(history_path) = perspt_core::paths::history_file() {
            if let Some(parent) = history_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&history_path, self.history.join("\n"));
        }

        // Build context
        let context: Vec<String> = self
            .messages
            .iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|m| {
                format!(
                    "{}: {}",
                    match m.role {
                        MessageRole::User => "User",
                        MessageRole::Assistant => "Assistant",
                        MessageRole::System => "System",
                    },
                    m.content
                )
            })
            .collect();

        // Start streaming
        self.is_streaming = true;
        self.streaming_buffer.clear();
        self.streaming_reasoning.clear();
        self.scroll_to_bottom();

        let (tx, rx) = mpsc::unbounded_channel();
        self.stream_rx = Some(rx);

        let provider = Arc::clone(&self.provider);
        let model = self.model.clone();

        tokio::spawn(async move {
            let _ = provider
                .generate_response_stream_to_channel(&model, &context.join("\n"), tx)
                .await;
        });

        Ok(())
    }

    /// Finalize streaming and add assistant message
    fn finalize_streaming(&mut self) {
        if !self.streaming_buffer.is_empty() || !self.streaming_reasoning.is_empty() {
            let mut msg = ChatMessage::assistant(self.streaming_buffer.clone());
            if !self.streaming_reasoning.is_empty() {
                msg.reasoning = Some(self.streaming_reasoning.clone());
            }
            self.push_message(msg);
        }
        self.streaming_buffer.clear();
        self.streaming_reasoning.clear();
        self.is_streaming = false;
    }

    /// Scroll up (disables auto-scroll)
    fn scroll_up(&mut self, n: usize) {
        self.auto_scroll = false; // User is manually scrolling
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll down
    fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(n);
        let max = self.total_visual_lines.saturating_sub(self.visible_height);
        if self.scroll_offset >= max {
            self.scroll_offset = max;
            self.auto_scroll = true; // Re-enable auto-scroll when at bottom
        }
    }

    /// Enable auto-scroll to bottom (actual scroll happens in render)
    fn scroll_to_bottom(&mut self) {
        self.auto_scroll = true;
    }

    /// Wrap a single line of text to fit within the given width.
    /// Returns a vector of wrapped lines (as owned Strings).
    fn wrap_text_to_width(text: &str, width: usize) -> Vec<String> {
        if width == 0 {
            return vec![text.to_string()];
        }

        let options = textwrap::Options::new(width).break_words(true);
        let wrapped = textwrap::wrap(text, options);
        let mut result: Vec<String> = wrapped.into_iter().map(|cow| cow.into_owned()).collect();

        if result.is_empty() {
            result.push(String::new());
        }

        result
    }

    /// Render the chat application
    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();

        // Calculate input height dynamically using visual wrapped lines
        let viewport_width = size.width.saturating_sub(2) as usize;
        let input_height = (self.input.line_count_wrapped(viewport_width) as u16 + 2).clamp(3, 10);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),            // Header
                Constraint::Min(10),              // Messages
                Constraint::Length(input_height), // Input
            ])
            .split(size);

        self.render_header(frame, chunks[0]);
        self.render_messages(frame, chunks[1]);
        self.render_input(frame, chunks[2]);
    }

    /// Render elegant header
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let header = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(96, 125, 139)))
            .title(Span::styled(
                format!(" {} Perspt Chat ", icons::ROCKET),
                Style::default()
                    .fg(Color::Rgb(129, 199, 132))
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(ratatui::layout::HorizontalAlignment::Left);

        let show_reasoning_str = if self.show_reasoning { "ON" } else { "OFF" };
        let model_display = format!(" {} │ Ctrl+R: Reasoning {} ", self.model, show_reasoning_str);
        let model_span = Span::styled(
            model_display.clone(),
            Style::default()
                .fg(Color::Rgb(176, 190, 197))
                .add_modifier(Modifier::ITALIC),
        );

        // Render block
        frame.render_widget(header, area);

        // Render model name and toggle on right side
        let model_area = Rect {
            x: area.x + area.width - model_display.len() as u16 - 4,
            y: area.y,
            width: model_display.len() as u16 + 3,
            height: 1,
        };
        frame.render_widget(Paragraph::new(model_span), model_area);
    }

    /// Render messages with markdown support and virtual scrolling
    fn render_messages(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(96, 125, 139)))
            .title(Span::styled(
                " Messages ",
                Style::default().fg(Color::Rgb(176, 190, 197)),
            ));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let viewport_width = inner.width as usize;
        let viewport_height = inner.height as usize;

        // Detect resize or first render
        let resize_detected = viewport_width != self.last_viewport_width || self.total_visual_lines == 0;
        self.last_viewport_width = viewport_width;
        self.visible_height = viewport_height;

        if resize_detected {
            for msg in &mut self.messages {
                msg.update_cache(viewport_width, self.show_reasoning);
            }
        }

        // Collect all pre-wrapped cached lines
        let mut visual_lines: Vec<Line<'static>> = Vec::new();
        for msg in &self.messages {
            for line in &msg.cached_visual_lines {
                visual_lines.push(line.clone());
            }
        }

        // Add streaming content on the fly
        if self.is_streaming && (!self.streaming_buffer.is_empty() || !self.streaming_reasoning.is_empty()) {
            let header_style = Style::default()
                .fg(Color::Rgb(144, 202, 249))
                .add_modifier(Modifier::BOLD);
            let content_style = Style::default().fg(Color::Rgb(189, 189, 189));

            visual_lines.push(Line::from(Span::styled(
                format!("━━━ {} Assistant ━━━", icons::ASSISTANT),
                header_style,
            )));

            // Parse thoughts from streaming content
            let (inline_thought, display_content) = ChatMessage::parse_inline_thought(&self.streaming_buffer);
            let combined_thought = match (&self.streaming_reasoning, &inline_thought) {
                (r, Some(i)) if !r.is_empty() => Some(format!("{}\n{}", r, i)),
                (r, None) if !r.is_empty() => Some(r.clone()),
                (_, Some(i)) => Some(i.clone()),
                (_, None) => None,
            };

            if self.show_reasoning {
                if let Some(ref thought) = combined_thought {
                    if !thought.is_empty() {
                        visual_lines.push(Line::from(Span::styled(
                            "  ⚡ Thought Process".to_string(),
                            Style::default()
                                .fg(Color::Rgb(255, 183, 77))
                                .add_modifier(Modifier::ITALIC | Modifier::BOLD),
                        )));
                        let reasoning_style = Style::default()
                            .fg(Color::Rgb(120, 144, 156))
                            .add_modifier(Modifier::ITALIC);
                        for line in thought.lines() {
                            let text = format!("    {}", line);
                            if text.width() <= viewport_width {
                                visual_lines.push(Line::from(Span::styled(text, reasoning_style)));
                            } else {
                                let wrapped = Self::wrap_text_to_width(&text, viewport_width);
                                for wrapped_line in wrapped {
                                    visual_lines.push(Line::from(Span::styled(wrapped_line, reasoning_style)));
                                }
                            }
                        }
                        visual_lines.push(Line::from(String::new()));
                    }
                }
            }

            // Pre-transpile math segments in the streaming display content
            let display_content_transpiled = ChatMessage::transpile_math_in_text(&display_content);

            let blocks = ChatMessage::parse_markdown_blocks(&display_content_transpiled);
            for block in blocks {
                match block {
                    ContentBlock::Markdown(text) => {
                        let rendered = tui_markdown::from_str(&text);
                        for line in rendered.lines {
                            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                            let parsed_line = ChatMessage::parse_line_to_spans(&text, content_style);
                            let wrapped = ChatMessage::wrap_line(parsed_line, viewport_width);
                            visual_lines.extend(wrapped);
                        }
                    }
                    ContentBlock::Table { headers, alignments, rows } => {
                        let table_lines = ChatMessage::render_table(headers, alignments, rows, viewport_width, content_style);
                        visual_lines.extend(table_lines);
                    }
                }
            }

            // Streaming cursor
            visual_lines.push(Line::from(Span::styled(
                "▌".to_string(),
                Style::default()
                    .fg(Color::Rgb(129, 212, 250))
                    .add_modifier(Modifier::SLOW_BLINK),
            )));
        }

        // Handle throbber when loading with empty buffers
        if self.is_streaming && self.streaming_buffer.is_empty() && self.streaming_reasoning.is_empty() {
            let throbber = Throbber::default()
                .label(" Thinking...")
                .style(Style::default().fg(Color::Rgb(255, 183, 77)));
            frame.render_stateful_widget(
                throbber,
                Rect::new(inner.x + 1, inner.y + 1, 20, 1),
                &mut self.throbber_state.clone(),
            );
        }

        // Calculate scroll position using visual line count
        let total_visual = visual_lines.len();
        self.total_visual_lines = total_visual;

        let max_scroll = total_visual.saturating_sub(viewport_height);

        let scroll_pos = if self.auto_scroll {
            max_scroll
        } else {
            self.scroll_offset.min(max_scroll)
        };

        // Update scroll_offset to actual position
        self.scroll_offset = scroll_pos;

        // Slice visible range and convert to Lines (virtual scrolling)
        let visible_lines: Vec<Line> = visual_lines
            .into_iter()
            .skip(scroll_pos)
            .take(viewport_height)
            .collect();

        // Render only the visible slice
        let paragraph = Paragraph::new(Text::from(visible_lines));
        frame.render_widget(paragraph, inner);

        // Scrollbar with accurate visual line count
        if total_visual > viewport_height {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::default().fg(Color::Rgb(96, 125, 139)));
            let mut state = ScrollbarState::new(total_visual).position(scroll_pos);
            frame.render_stateful_widget(scrollbar, area.inner(Margin::new(0, 1)), &mut state);
        }
    }

    /// Render input area
    fn render_input(&self, frame: &mut Frame, area: Rect) {
        if self.is_streaming {
            // Show streaming indicator
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(96, 125, 139)))
                .title(Span::styled(
                    " Receiving response... ",
                    Style::default().fg(Color::Rgb(255, 183, 77)),
                ));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let text = Paragraph::new("Press Ctrl+C to cancel")
                .style(Style::default().fg(Color::Rgb(120, 144, 156)));
            frame.render_widget(text, inner);
        } else {
            // Render input with hint
            self.input
                .render(frame, area, "Enter=send │ Ctrl+J=newline");
        }
    }
}

/// Run the chat TUI
pub async fn run_chat_tui(provider: GenAIProvider, model: String) -> Result<()> {
    use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
    use ratatui::crossterm::execute;
    use std::io::stdout;

    // Enable mouse capture
    execute!(stdout(), EnableMouseCapture)?;

    let mut terminal = ratatui::init();
    let mut app = ChatApp::new(provider, model);

    let result = app.run(&mut terminal).await;

    // Restore terminal
    ratatui::restore();
    execute!(stdout(), DisableMouseCapture)?;

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use perspt_core::GenAIProvider;
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers};

    #[tokio::test]
    async fn test_slash_commands_in_tui() {
        let provider = GenAIProvider::new().unwrap_or_else(|_| {
            GenAIProvider::new_with_config(Some("openai"), Some("dummy_key")).unwrap()
        });
        let mut app = ChatApp::new(provider, "gpt-4".to_string());

        // Test /help command
        app.input.set_text("/help");
        app.handle_terminal_event(CrosstermEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert_eq!(app.input.text(), "");
        assert!(app.messages.iter().any(|m| m.content.contains("Available Slash Commands:")));

        // Test /model switching
        app.input.set_text("/model custom-gemma");
        app.handle_terminal_event(CrosstermEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert_eq!(app.input.text(), "");
        assert_eq!(app.model, "custom-gemma");
        assert!(app.messages.iter().any(|m| m.content.contains("Switched model to: custom-gemma")));

        // Test /clear command
        app.input.set_text("/clear");
        app.handle_terminal_event(CrosstermEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert_eq!(app.input.text(), "");
        assert_eq!(app.messages.len(), 1);
        assert!(app.messages[0].content.contains("Conversation history cleared."));

        // Test /save command
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_perspt_conv.md");
        let test_file_str = test_file.to_string_lossy().to_string();
        
        app.input.set_text(&format!("/save {}", test_file_str));
        app.handle_terminal_event(CrosstermEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert_eq!(app.input.text(), "");
        assert!(test_file.exists());
        let _ = std::fs::remove_file(test_file);
    }

    #[test]
    fn test_parse_inline_thought() {
        let content = "Hello world <think>I should think about this.</think> Actual answer";
        let (thought, remaining) = ChatMessage::parse_inline_thought(content);
        assert_eq!(thought, Some("I should think about this.".to_string()));
        assert_eq!(remaining, "Hello world  Actual answer");

        let content_no_thought = "Just some content without thinking tags";
        let (thought, remaining) = ChatMessage::parse_inline_thought(content_no_thought);
        assert_eq!(thought, None);
        assert_eq!(remaining, "Just some content without thinking tags");

        let unclosed_thought = "Thinking <think>I am currently thinking...";
        let (thought, remaining) = ChatMessage::parse_inline_thought(unclosed_thought);
        assert_eq!(thought, Some("I am currently thinking...".to_string()));
        assert_eq!(remaining, "Thinking ");
    }

    #[test]
    fn test_math_formula_rendering() {
        // Test transpile_latex_to_unicode
        let latex = r"E = m c^2 + \alpha \ge \beta";
        let unicode = ChatMessage::transpile_latex_to_unicode(latex);
        assert_eq!(unicode, "E = m c² + α ≥ β");

        // Test math wrappers like \mathbf, \text, fractions, square roots
        let complex_latex = r"\mathbf{3} + \text{hello} + \frac{1}{\sqrt{2}} + x_{max} + e^{i\pi}";
        let complex_unicode = ChatMessage::transpile_latex_to_unicode(complex_latex);
        assert_eq!(complex_unicode, "3 + hello + (1)/(√(2)) + x_max + eⁱπ");

        // Test transpile_math_in_text
        let text = "Formula is $E = m c^2$ and block is $$\\alpha + \\beta = \\gamma$$";
        let transpiled = ChatMessage::transpile_math_in_text(text);
        assert_eq!(transpiled, "Formula is $E = m c²$ and block is $$α + β = γ$$");

        // Test parse_line_to_spans
        let line = ChatMessage::parse_line_to_spans("Formula is $E = m c²$ end.", Style::default());
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].content, "Formula is ");
        assert_eq!(line.spans[1].content, "E = m c²");
        assert_eq!(line.spans[2].content, " end.");

        // Test mathbb (blackboard bold)
        assert_eq!(ChatMessage::transpile_latex_to_unicode(r"\mathbb{N}"), "ℕ");
        assert_eq!(ChatMessage::transpile_latex_to_unicode(r"\mathbb{R}"), "ℝ");
        assert_eq!(ChatMessage::transpile_latex_to_unicode(r"\mathbb{Z}"), "ℤ");
        assert_eq!(ChatMessage::transpile_latex_to_unicode(r"\mathbb{C}"), "ℂ");
        assert_eq!(ChatMessage::transpile_latex_to_unicode(r"\mathbb{Q}"), "ℚ");

        // Test pmod (critical: must not collide with \pm)
        assert_eq!(ChatMessage::transpile_latex_to_unicode(r"n \equiv 0 \pmod{2}"), "n ≡ 0  (mod 2)");
        assert_eq!(ChatMessage::transpile_latex_to_unicode(r"\pm"), "±");

        // Test begin/end environments are stripped
        assert_eq!(ChatMessage::transpile_latex_to_unicode(r"\begin{cases} a \\ b \end{cases}"), " a  b ");

        // Test dots
        assert_eq!(ChatMessage::transpile_latex_to_unicode(r"\dots"), "…");
        assert_eq!(ChatMessage::transpile_latex_to_unicode(r"\cdots"), "⋯");
        assert_eq!(ChatMessage::transpile_latex_to_unicode(r"\ldots"), "…");
    }

    #[test]
    fn test_wrap_line_with_math_formulas() {
        let text = "This is a long formula $$E = m c²$$ that needs wrapping.";
        let line = ChatMessage::parse_line_to_spans(text, Style::default());
        let wrapped = ChatMessage::wrap_line(line, 20);

        // Verify it wrapped into multiple lines
        assert!(wrapped.len() > 1);

        // Verify that the math styled spans have the correct math style (italic/bold)
        let mut math_spans_found = 0;
        let math_color = Color::Rgb(129, 212, 250);
        for wrapped_line in wrapped {
            for span in wrapped_line.spans {
                if span.style.fg == Some(math_color) {
                    math_spans_found += 1;
                    assert!(span.style.add_modifier.contains(Modifier::ITALIC));
                    assert!(span.style.add_modifier.contains(Modifier::BOLD));
                }
            }
        }
        assert!(math_spans_found > 0);
    }

    #[test]
    fn test_markdown_table_rendering() {
        // Test is_separator_line
        assert!(ChatMessage::is_separator_line("|---|---|"));
        assert!(ChatMessage::is_separator_line("| :--- | :---: | ---: |"));
        assert!(!ChatMessage::is_separator_line("| normal | line |"));
        
        // Test split_table_row
        assert_eq!(ChatMessage::split_table_row("| Header 1 | Header 2 |"), vec!["Header 1", "Header 2"]);
        assert_eq!(ChatMessage::split_table_row("Col 1 | Col 2"), vec!["Col 1", "Col 2"]);
        assert_eq!(ChatMessage::split_table_row("| escaped\\|pipe | second |"), vec!["escaped|pipe", "second"]);
        
        // Test parse_alignment
        assert_eq!(ChatMessage::parse_alignment(":---"), TableAlign::Left);
        assert_eq!(ChatMessage::parse_alignment(":---:"), TableAlign::Center);
        assert_eq!(ChatMessage::parse_alignment("---:"), TableAlign::Right);
        assert_eq!(ChatMessage::parse_alignment("---"), TableAlign::Left);
        
        // Test parse_markdown_blocks
        let md = "Some text\n\n| H1 | H2 |\n|---|---|\n| v1 | v2 |\n\nFooter text";
        let blocks = ChatMessage::parse_markdown_blocks(md);
        assert_eq!(blocks.len(), 3);
        
        // Test render_table
        let headers = vec!["Col A".to_string(), "Col B".to_string()];
        let alignments = vec![TableAlign::Left, TableAlign::Center];
        let rows = vec![vec!["1".to_string(), "2".to_string()]];
        let table_lines = ChatMessage::render_table(headers, alignments, rows, 80, Style::default());
        
        assert_eq!(table_lines.len(), 5); // top, header, separator, row, bottom
        
        // Check borders are drawn correctly
        let top_str: String = table_lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(top_str.contains("┌"));
        assert!(top_str.contains("┬"));
        assert!(top_str.contains("┐"));
        
        let header_str: String = table_lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(header_str.contains("Col A"));
        assert!(header_str.contains("Col B"));
        assert!(header_str.contains("│"));

        // Test multi-line cell wrapping and <br> splitting
        let long_headers = vec!["Col A".to_string(), "Col B".to_string()];
        let long_alignments = vec![TableAlign::Left, TableAlign::Left];
        let long_rows = vec![vec![
            "Short".to_string(),
            "Line 1<br>Line 2 that is very long indeed".to_string(),
        ]];
        
        // Render with a small viewport to force wrapping
        let wrapped_table_lines = ChatMessage::render_table(
            long_headers,
            long_alignments,
            long_rows,
            30,
            Style::default(),
        );
        
        // Due to wrapping of Col B, the row should span multiple sub-rows, increasing total line count
        assert!(wrapped_table_lines.len() > 5);
    }
}
