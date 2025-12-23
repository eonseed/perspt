//! Diff Viewer Component
//!
//! Syntax-highlighted diff display for file changes.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

/// A single diff hunk
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// Original file path
    pub file_path: String,
    /// Lines with change type
    pub lines: Vec<DiffLine>,
}

/// A diff line with its type
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// Line content
    pub content: String,
    /// Line type
    pub line_type: DiffLineType,
    /// Line number (if applicable)
    pub line_number: Option<usize>,
}

/// Type of diff line
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineType {
    /// Unchanged context line
    Context,
    /// Added line
    Added,
    /// Removed line
    Removed,
    /// Header line (file path, etc.)
    Header,
}

impl DiffLine {
    pub fn new(content: &str, line_type: DiffLineType) -> Self {
        Self {
            content: content.to_string(),
            line_type,
            line_number: None,
        }
    }
}

#[derive(Default)]
pub struct DiffViewer {
    /// Diff hunks to display
    pub hunks: Vec<DiffHunk>,
    /// Current scroll offset
    pub scroll: usize,
    /// Currently selected hunk index
    pub selected_hunk: usize,
}

impl DiffViewer {
    /// Create a new diff viewer
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a unified diff string
    pub fn parse_diff(&mut self, diff_text: &str) {
        self.hunks.clear();
        let mut current_hunk: Option<DiffHunk> = None;

        for line in diff_text.lines() {
            if line.starts_with("diff --git") || line.starts_with("---") || line.starts_with("+++")
            {
                if let Some(hunk) = current_hunk.take() {
                    self.hunks.push(hunk);
                }
                current_hunk = Some(DiffHunk {
                    file_path: line.to_string(),
                    lines: vec![DiffLine::new(line, DiffLineType::Header)],
                });
            } else if line.starts_with("@@") {
                if let Some(ref mut hunk) = current_hunk {
                    hunk.lines.push(DiffLine::new(line, DiffLineType::Header));
                }
            } else if line.starts_with('+') {
                if let Some(ref mut hunk) = current_hunk {
                    hunk.lines.push(DiffLine::new(line, DiffLineType::Added));
                }
            } else if line.starts_with('-') {
                if let Some(ref mut hunk) = current_hunk {
                    hunk.lines.push(DiffLine::new(line, DiffLineType::Removed));
                }
            } else if let Some(ref mut hunk) = current_hunk {
                hunk.lines.push(DiffLine::new(line, DiffLineType::Context));
            }
        }

        if let Some(hunk) = current_hunk {
            self.hunks.push(hunk);
        }
    }

    /// Scroll up
    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    /// Scroll down
    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    /// Next hunk
    pub fn next_hunk(&mut self) {
        if self.selected_hunk < self.hunks.len().saturating_sub(1) {
            self.selected_hunk += 1;
        }
    }

    /// Previous hunk
    pub fn prev_hunk(&mut self) {
        self.selected_hunk = self.selected_hunk.saturating_sub(1);
    }

    /// Render the diff viewer
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let lines: Vec<Line> = self
            .hunks
            .iter()
            .flat_map(|hunk| {
                hunk.lines.iter().map(|line| {
                    let style = match line.line_type {
                        DiffLineType::Added => Style::default().fg(Color::Green),
                        DiffLineType::Removed => Style::default().fg(Color::Red),
                        DiffLineType::Header => Style::default().fg(Color::Cyan),
                        DiffLineType::Context => Style::default().fg(Color::DarkGray),
                    };

                    let prefix = match line.line_type {
                        DiffLineType::Added => "+ ",
                        DiffLineType::Removed => "- ",
                        DiffLineType::Header => "  ",
                        DiffLineType::Context => "  ",
                    };

                    Line::from(vec![
                        Span::styled(prefix, style),
                        Span::styled(&line.content, style),
                    ])
                })
            })
            .collect();

        let total_lines = lines.len();
        let visible_lines = area.height.saturating_sub(2) as usize;
        let max_scroll = total_lines.saturating_sub(visible_lines);
        let scroll = self.scroll.min(max_scroll);

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(format!("📝 Diff ({} hunks)", self.hunks.len()))
                    .borders(Borders::ALL),
            )
            .scroll((scroll as u16, 0));

        frame.render_widget(para, area);

        // Scrollbar
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
