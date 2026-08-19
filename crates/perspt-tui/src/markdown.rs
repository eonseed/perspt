//! Markdown, table, and LaTeX-math rendering for the chat TUI.
//!
//! Pure functions extracted from `chat_app` (PSP-1 decomposition): LaTeX →
//! Unicode transpilation, `$`/`\(`/`\[` math normalization, markdown block
//! and table parsing, and line wrapping. `tui_markdown` renders the
//! markdown blocks themselves; this module owns everything around it.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub use crate::latex::*;

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

/// Normalize LaTeX display/inline delimiters to the `$` forms the math
/// pipeline understands: `\[...\]` → `$$...$$`, `\(...\)` → `$...$`.
/// Modern models emit the backslash forms almost exclusively. Fenced
/// code segments pass through untouched — regex escapes legitimately
/// contain `\(` and `\[`.
pub fn normalize_math_delimiters(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    for (index, segment) in content.split("```").enumerate() {
        if index > 0 {
            out.push_str("```");
        }
        if index % 2 == 1 {
            out.push_str(segment);
            continue;
        }
        out.push_str(
            &segment
                .replace("\\[", "$$")
                .replace("\\]", "$$")
                .replace("\\(", "$")
                .replace("\\)", "$"),
        );
    }
    out
}

/// Pre-transpile math segments in string before soft wrapping
pub fn transpile_math_in_text(content: &str) -> String {
    let normalized = normalize_math_delimiters(content);
    let mut result = String::new();
    let mut remaining = normalized.as_str();

    while let Some(start_idx) = remaining.find('$') {
        result.push_str(&remaining[..start_idx]);
        let after_start = &remaining[start_idx + 1..];

        if let Some(after_double) = after_start.strip_prefix('$') {
            // Block math $$...$$
            if let Some(end_idx) = after_double.find("$$") {
                let math_content = &after_double[..end_idx];
                let transpiled = transpile_latex_to_unicode(math_content);
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
                let transpiled = transpile_latex_to_unicode(math_content);
                result.push_str(&format!("${}$", transpiled));
                remaining = &after_start[end_idx + 1..];
            } else {
                result.push('$');
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
        if let Some(after_double) = after_start.strip_prefix('$') {
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
        let is_next_separator = has_next && is_separator_line(lines[i + 1]);

        if is_header_candidate && is_next_separator {
            // Flush current markdown if not empty
            if !current_markdown.is_empty() {
                blocks.push(ContentBlock::Markdown(current_markdown.clone()));
                current_markdown.clear();
            }

            let header_line = line;
            let separator_line = lines[i + 1];

            let headers = split_table_row(header_line);
            let sep_cells = split_table_row(separator_line);

            let alignments: Vec<TableAlign> =
                sep_cells.iter().map(|cell| parse_alignment(cell)).collect();

            let mut rows = Vec::new();
            i += 2; // skip header and separator

            while i < lines.len() {
                let data_line = lines[i];
                let trimmed_data = data_line.trim();

                if trimmed_data.starts_with("```") {
                    break;
                }

                if data_line.contains('|') {
                    if is_separator_line(data_line) {
                        break;
                    }
                    rows.push(split_table_row(data_line));
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
        return Line::from(vec![Span::styled(
            "…",
            Style::default().fg(Color::Rgb(120, 144, 156)),
        )]);
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
        new_spans.push(Span::styled(
            "…",
            Style::default().fg(Color::Rgb(120, 144, 156)),
        ));
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
            let parsed_line = parse_line_to_spans(part, cell_style);
            let wrapped = wrap_line(parsed_line, w);
            col_lines.extend(wrapped);
        }
        cell_wrapped_lines.push(col_lines);
    }

    // Determine max line count for this row
    let line_count = cell_wrapped_lines
        .iter()
        .map(|lines| lines.len())
        .max()
        .unwrap_or(1);

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

            let line_width = cell_line
                .spans
                .iter()
                .map(|s| s.content.width())
                .sum::<usize>();
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

/// Column widths for one table: natural widths clamped to a third of
/// the viewport, then proportionally scaled down when the drawn table
/// would overflow it.
fn table_column_widths(
    headers: &[String],
    formatted_rows: &[Vec<String>],
    col_count: usize,
    viewport_width: usize,
    content_style: Style,
) -> Vec<usize> {
    // Calculate max natural width of columns, clamping them reasonably
    let max_natural_width = (viewport_width / 3).max(20);
    let mut col_widths = vec![0; col_count];
    for (i, h) in headers.iter().enumerate() {
        let header_line = parse_line_to_spans(h, content_style);
        let header_width = header_line
            .spans
            .iter()
            .map(|s| s.content.width())
            .sum::<usize>();
        col_widths[i] = col_widths[i].max(header_width.min(max_natural_width));
    }
    for row in formatted_rows {
        for (i, cell) in row.iter().enumerate() {
            let cell_line = parse_line_to_spans(cell, content_style);
            let cell_width = cell_line
                .spans
                .iter()
                .map(|s| s.content.width())
                .sum::<usize>();
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
            return new_widths;
        }
    }

    col_widths
}

/// Render a complete GFM table with Unicode borders and alignments
/// One horizontal table border line: `(left, joint, right)` picks the
/// top (┌┬┐), separator (├┼┤), or bottom (└┴┘) corner set.
fn table_border(
    col_widths: &[usize],
    corners: (&str, &str, &str),
    border_style: Style,
) -> Line<'static> {
    let mut spans = vec![Span::styled(corners.0.to_string(), border_style)];
    for (idx, &w) in col_widths.iter().enumerate() {
        spans.push(Span::styled("─".repeat(w + 2), border_style));
        if idx + 1 < col_widths.len() {
            spans.push(Span::styled(corners.1.to_string(), border_style));
        }
    }
    spans.push(Span::styled(corners.2.to_string(), border_style));
    Line::from(spans)
}

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

    let col_widths = table_column_widths(
        &headers,
        &formatted_rows,
        col_count,
        viewport_width,
        content_style,
    );

    // Border styling and header styling
    let border_style = Style::default().fg(Color::Rgb(100, 116, 139));
    let header_style = content_style.add_modifier(Modifier::BOLD);

    let mut lines = Vec::new();

    lines.push(table_border(&col_widths, ("┌", "┬", "┐"), border_style));

    // Render headers
    lines.extend(render_table_row(
        &headers,
        &col_widths,
        &alignments,
        border_style,
        header_style,
    ));

    // Render separator and data rows
    if !formatted_rows.is_empty() {
        lines.push(table_border(&col_widths, ("├", "┼", "┤"), border_style));

        for row in &formatted_rows {
            lines.extend(render_table_row(
                row,
                &col_widths,
                &alignments,
                border_style,
                content_style,
            ));
        }
    }

    lines.push(table_border(&col_widths, ("└", "┴", "┘"), border_style));

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backslash_delimited_math_renders_like_dollar_math() {
        // The live-model shape: modern models emit \( \) and \[ \] almost
        // exclusively (qwen-3.8, gemini); the pipeline must treat them
        // exactly like the historic $ forms.
        let content = "For \\( ax^2 + bx + c = 0 \\) with \\( a \\ne 0 \\),\n\n\
                       \\[\nx = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}\n\\]";
        let transpiled = crate::markdown::transpile_math_in_text(content);
        assert!(transpiled.contains('$'), "delimiters normalize to $ forms");
        assert!(transpiled.contains('±'), "\\pm transpiles: {transpiled}");
        assert!(transpiled.contains('√'), "\\sqrt transpiles: {transpiled}");
        assert!(transpiled.contains('≠'), "\\ne transpiles: {transpiled}");
        assert!(
            !transpiled.contains("\\frac") && !transpiled.contains("\\["),
            "no raw LaTeX survives: {transpiled}"
        );

        // Fenced code is untouched: regex escapes legitimately contain \(.
        let code = "```rust\nlet re = Regex::new(r\"\\(x\\)\");\n```";
        assert_eq!(
            crate::markdown::transpile_math_in_text(code),
            code,
            "fenced code passes through byte-identical"
        );
    }

    #[test]
    fn test_math_formula_rendering() {
        // Test transpile_latex_to_unicode
        let latex = r"E = m c^2 + \alpha \ge \beta";
        let unicode = crate::markdown::transpile_latex_to_unicode(latex);
        assert_eq!(unicode, "E = m c² + α ≥ β");

        // Test math wrappers like \mathbf, \text, fractions, square roots
        let complex_latex = r"\mathbf{3} + \text{hello} + \frac{1}{\sqrt{2}} + x_{max} + e^{i\pi}";
        let complex_unicode = crate::markdown::transpile_latex_to_unicode(complex_latex);
        assert_eq!(complex_unicode, "3 + hello + (1)/(√(2)) + x_max + eⁱπ");

        // Test transpile_math_in_text
        let text = "Formula is $E = m c^2$ and block is $$\\alpha + \\beta = \\gamma$$";
        let transpiled = crate::markdown::transpile_math_in_text(text);
        assert_eq!(
            transpiled,
            "Formula is $E = m c²$ and block is $$α + β = γ$$"
        );

        // Test parse_line_to_spans
        let line =
            crate::markdown::parse_line_to_spans("Formula is $E = m c²$ end.", Style::default());
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].content, "Formula is ");
        assert_eq!(line.spans[1].content, "E = m c²");
        assert_eq!(line.spans[2].content, " end.");

        // Test mathbb (blackboard bold)
        assert_eq!(
            crate::markdown::transpile_latex_to_unicode(r"\mathbb{N}"),
            "ℕ"
        );
        assert_eq!(
            crate::markdown::transpile_latex_to_unicode(r"\mathbb{R}"),
            "ℝ"
        );
        assert_eq!(
            crate::markdown::transpile_latex_to_unicode(r"\mathbb{Z}"),
            "ℤ"
        );
        assert_eq!(
            crate::markdown::transpile_latex_to_unicode(r"\mathbb{C}"),
            "ℂ"
        );
        assert_eq!(
            crate::markdown::transpile_latex_to_unicode(r"\mathbb{Q}"),
            "ℚ"
        );

        // Test pmod (critical: must not collide with \pm)
        assert_eq!(
            crate::markdown::transpile_latex_to_unicode(r"n \equiv 0 \pmod{2}"),
            "n ≡ 0  (mod 2)"
        );
        assert_eq!(crate::markdown::transpile_latex_to_unicode(r"\pm"), "±");

        // Test begin/end environments are stripped
        assert_eq!(
            crate::markdown::transpile_latex_to_unicode(r"\begin{cases} a \\ b \end{cases}"),
            " a  b "
        );

        // Test dots
        assert_eq!(crate::markdown::transpile_latex_to_unicode(r"\dots"), "…");
        assert_eq!(crate::markdown::transpile_latex_to_unicode(r"\cdots"), "⋯");
        assert_eq!(crate::markdown::transpile_latex_to_unicode(r"\ldots"), "…");
    }

    #[test]
    fn test_wrap_line_with_math_formulas() {
        let text = "This is a long formula $$E = m c²$$ that needs wrapping.";
        let line = crate::markdown::parse_line_to_spans(text, Style::default());
        let wrapped = crate::markdown::wrap_line(line, 20);

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
        assert!(crate::markdown::is_separator_line("|---|---|"));
        assert!(crate::markdown::is_separator_line(
            "| :--- | :---: | ---: |"
        ));
        assert!(!crate::markdown::is_separator_line("| normal | line |"));

        // Test split_table_row
        assert_eq!(
            crate::markdown::split_table_row("| Header 1 | Header 2 |"),
            vec!["Header 1", "Header 2"]
        );
        assert_eq!(
            crate::markdown::split_table_row("Col 1 | Col 2"),
            vec!["Col 1", "Col 2"]
        );
        assert_eq!(
            crate::markdown::split_table_row("| escaped\\|pipe | second |"),
            vec!["escaped|pipe", "second"]
        );

        // Test parse_alignment
        assert_eq!(crate::markdown::parse_alignment(":---"), TableAlign::Left);
        assert_eq!(
            crate::markdown::parse_alignment(":---:"),
            TableAlign::Center
        );
        assert_eq!(crate::markdown::parse_alignment("---:"), TableAlign::Right);
        assert_eq!(crate::markdown::parse_alignment("---"), TableAlign::Left);

        // Test parse_markdown_blocks
        let md = "Some text\n\n| H1 | H2 |\n|---|---|\n| v1 | v2 |\n\nFooter text";
        let blocks = crate::markdown::parse_markdown_blocks(md);
        assert_eq!(blocks.len(), 3);

        // Test render_table
        let headers = vec!["Col A".to_string(), "Col B".to_string()];
        let alignments = vec![TableAlign::Left, TableAlign::Center];
        let rows = vec![vec!["1".to_string(), "2".to_string()]];
        let table_lines =
            crate::markdown::render_table(headers, alignments, rows, 80, Style::default());

        assert_eq!(table_lines.len(), 5); // top, header, separator, row, bottom

        // Check borders are drawn correctly
        let top_str: String = table_lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(top_str.contains("┌"));
        assert!(top_str.contains("┬"));
        assert!(top_str.contains("┐"));

        let header_str: String = table_lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
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
        let wrapped_table_lines = crate::markdown::render_table(
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
