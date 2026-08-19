//! LaTeX → Unicode transpilation for terminal math rendering (PSP-1
//! decomposition from `markdown`): superscripts, subscripts, blackboard
//! bold, fractions, roots, wrappers, and the symbol table.

pub fn replace_superscripts(text: &str) -> String {
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
                '0' => '⁰',
                '1' => '¹',
                '2' => '²',
                '3' => '³',
                '4' => '⁴',
                '5' => '⁵',
                '6' => '⁶',
                '7' => '⁷',
                '8' => '⁸',
                '9' => '⁹',
                '+' => '⁺',
                '-' => '⁻',
                '=' => '⁼',
                '(' => '⁽',
                ')' => '⁾',
                'n' => 'ⁿ',
                'i' => 'ⁱ',
                'x' => 'ˣ',
                'y' => 'ʸ',
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
pub fn replace_subscripts(text: &str) -> String {
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
                '0' => '₀',
                '1' => '₁',
                '2' => '₂',
                '3' => '₃',
                '4' => '₄',
                '5' => '₅',
                '6' => '₆',
                '7' => '₇',
                '8' => '₈',
                '9' => '₉',
                '+' => '₊',
                '-' => '₋',
                '=' => '₌',
                '(' => '₍',
                ')' => '₎',
                'a' => 'ₐ',
                'e' => 'ₑ',
                'h' => 'ₕ',
                'i' => 'ᵢ',
                'j' => 'ⱼ',
                'k' => 'ₖ',
                'l' => 'ₗ',
                'm' => 'ₘ',
                'n' => 'ₙ',
                'o' => 'ₒ',
                'p' => 'ₚ',
                'r' => 'ᵣ',
                's' => 'ₛ',
                't' => 'ₜ',
                'u' => 'ᵤ',
                'v' => 'ᵥ',
                'x' => 'ₓ',
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
pub fn strip_latex_wrappers(text: &str) -> String {
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

/// Blackboard-bold Unicode for `\mathbb{X}`.
const BB_MAP: &[(&str, &str)] = &[
    ("A", "𝔸"),
    ("B", "𝔹"),
    ("C", "ℂ"),
    ("D", "𝔻"),
    ("E", "𝔼"),
    ("F", "𝔽"),
    ("G", "𝔾"),
    ("H", "ℍ"),
    ("I", "𝕀"),
    ("J", "𝕁"),
    ("K", "𝕂"),
    ("L", "𝕃"),
    ("M", "𝕄"),
    ("N", "ℕ"),
    ("O", "𝕆"),
    ("P", "ℙ"),
    ("Q", "ℚ"),
    ("R", "ℝ"),
    ("S", "𝕊"),
    ("T", "𝕋"),
    ("U", "𝕌"),
    ("V", "𝕍"),
    ("W", "𝕎"),
    ("X", "𝕏"),
    ("Y", "𝕐"),
    ("Z", "ℤ"),
];

/// Transpile \\mathbb{X} to blackboard bold Unicode (ℕ, ℤ, ℝ, ℚ, ℂ, etc.)
pub fn strip_latex_mathbb(text: &str) -> String {
    let mut result = text.to_string();
    let mut changed = true;

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
                    if let Some((_, bb)) = BB_MAP.iter().find(|(k, _)| *k == s) {
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
pub fn strip_latex_pmod(text: &str) -> String {
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
pub fn strip_latex_environments(text: &str) -> String {
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
pub fn strip_latex_fractions(text: &str) -> String {
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
pub fn transpile_sqrt(text: &str) -> String {
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
pub fn strip_sub_super_braces(text: &str) -> String {
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

/// LaTeX symbol substitutions, ordered longest-first so shorter
/// patterns never greedily match inside longer ones
/// (`\partial` before `\par`, `\implies` before `\in`).
const SYMBOL_SUBSTITUTIONS: &[(&str, &str)] = &[
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

/// Transpile common LaTeX macros into high-fidelity mathematical Unicode symbols
pub fn transpile_latex_to_unicode(math: &str) -> String {
    let mut result = math.to_string();

    // Handle structural LaTeX directives first (brace-based)
    result = transpile_sqrt(&result);
    result = strip_latex_fractions(&result);
    result = strip_latex_mathbb(&result);
    result = strip_latex_pmod(&result);
    result = strip_latex_environments(&result);
    result = strip_latex_wrappers(&result);
    result = strip_sub_super_braces(&result);

    for (latex, unicode) in SYMBOL_SUBSTITUTIONS {
        result = result.replace(latex, unicode);
    }

    result = replace_superscripts(&result);
    result = replace_subscripts(&result);
    result = result.replace("\\\\", "");
    result = result.replace("\\", "");
    result
}
