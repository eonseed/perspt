//! Diff Viewer Component
//!
//! Rich diff display with syntax highlighting and line numbers.

use crate::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Tabs},
    Frame,
};
use similar::{ChangeTag, TextDiff};

/// Display mode for diffs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffViewMode {
    #[default]
    Unified,
    SideBySide,
}

/// A single diff hunk
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// Original file path
    pub file_path: String,
    /// File extension (for syntax highlighting)
    pub extension: Option<String>,
    /// Lines with change type
    pub lines: Vec<DiffLine>,
    /// Original line number start
    pub old_start: usize,
    /// New line number start
    pub new_start: usize,
}

/// A diff line with its type
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// Line content
    pub content: String,
    /// Line type
    pub line_type: DiffLineType,
    /// Old line number (if applicable)
    pub old_line_number: Option<usize>,
    /// New line number (if applicable)
    pub new_line_number: Option<usize>,
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
    /// Hunk header (@@...@@)
    HunkHeader,
}

impl DiffLine {
    pub fn new(content: &str, line_type: DiffLineType) -> Self {
        Self {
            content: content.to_string(),
            line_type,
            old_line_number: None,
            new_line_number: None,
        }
    }

    pub fn with_line_numbers(
        content: &str,
        line_type: DiffLineType,
        old: Option<usize>,
        new: Option<usize>,
    ) -> Self {
        Self {
            content: content.to_string(),
            line_type,
            old_line_number: old,
            new_line_number: new,
        }
    }
}

/// Enhanced diff viewer with syntax highlighting
pub struct DiffViewer {
    /// Diff hunks to display
    pub hunks: Vec<DiffHunk>,
    /// Current scroll offset
    pub scroll: usize,
    /// Currently selected hunk index
    pub selected_hunk: usize,
    /// View mode
    pub view_mode: DiffViewMode,
    /// Theme for styling
    theme: Theme,
    /// Total line count (cached for scrolling)
    total_lines: usize,
}

impl Default for DiffViewer {
    fn default() -> Self {
        Self {
            hunks: Vec::new(),
            scroll: 0,
            selected_hunk: 0,
            view_mode: DiffViewMode::Unified,
            theme: Theme::default(),
            total_lines: 0,
        }
    }
}

impl DiffViewer {
    /// Create a new diff viewer
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute diff between two strings using `similar`
    pub fn compute_diff(&mut self, file_path: &str, old_content: &str, new_content: &str) {
        self.hunks.clear();

        let diff = TextDiff::from_lines(old_content, new_content);
        let extension = file_path.rsplit('.').next().map(String::from);

        let mut current_hunk = DiffHunk {
            file_path: file_path.to_string(),
            extension: extension.clone(),
            lines: vec![DiffLine::new(
                &format!("diff --git a/{} b/{}", file_path, file_path),
                DiffLineType::Header,
            )],
            old_start: 1,
            new_start: 1,
        };

        let mut old_line = 1usize;
        let mut new_line = 1usize;

        for change in diff.iter_all_changes() {
            let (line_type, old_num, new_num) = match change.tag() {
                ChangeTag::Delete => {
                    let num = old_line;
                    old_line += 1;
                    (DiffLineType::Removed, Some(num), None)
                }
                ChangeTag::Insert => {
                    let num = new_line;
                    new_line += 1;
                    (DiffLineType::Added, None, Some(num))
                }
                ChangeTag::Equal => {
                    let o = old_line;
                    let n = new_line;
                    old_line += 1;
                    new_line += 1;
                    (DiffLineType::Context, Some(o), Some(n))
                }
            };

            // Remove trailing newline for display
            let content = change.value().trim_end_matches('\n');
            current_hunk.lines.push(DiffLine::with_line_numbers(
                content, line_type, old_num, new_num,
            ));
        }

        if !current_hunk.lines.is_empty() {
            self.hunks.push(current_hunk);
        }

        self.update_total_lines();
    }

    /// Parse a unified diff string
    pub fn parse_diff(&mut self, diff_text: &str) {
        self.hunks.clear();
        let mut current_hunk: Option<DiffHunk> = None;
        let mut old_line = 1usize;
        let mut new_line = 1usize;

        for line in diff_text.lines() {
            if line.starts_with("diff --git") {
                if let Some(hunk) = current_hunk.take() {
                    self.hunks.push(hunk);
                }
                let file_path = line.split(" b/").nth(1).unwrap_or("unknown").to_string();
                let extension = file_path.rsplit('.').next().map(String::from);
                current_hunk = Some(DiffHunk {
                    file_path,
                    extension,
                    lines: vec![DiffLine::new(line, DiffLineType::Header)],
                    old_start: 1,
                    new_start: 1,
                });
                old_line = 1;
                new_line = 1;
            } else if line.starts_with("---") || line.starts_with("+++") {
                if let Some(ref mut hunk) = current_hunk {
                    hunk.lines.push(DiffLine::new(line, DiffLineType::Header));
                }
            } else if line.starts_with("@@") {
                // Parse hunk header to get line numbers
                if let Some(ref mut hunk) = current_hunk {
                    hunk.lines
                        .push(DiffLine::new(line, DiffLineType::HunkHeader));
                    // Parse @@ -old_start,old_count +new_start,new_count @@
                    if let Some(nums) = parse_hunk_header(line) {
                        old_line = nums.0;
                        new_line = nums.2;
                        hunk.old_start = nums.0;
                        hunk.new_start = nums.2;
                    }
                }
            } else if let Some(ref mut hunk) = current_hunk {
                let (line_type, old_num, new_num) = if line.starts_with('+') {
                    let n = new_line;
                    new_line += 1;
                    (DiffLineType::Added, None, Some(n))
                } else if line.starts_with('-') {
                    let o = old_line;
                    old_line += 1;
                    (DiffLineType::Removed, Some(o), None)
                } else {
                    let o = old_line;
                    let n = new_line;
                    old_line += 1;
                    new_line += 1;
                    (DiffLineType::Context, Some(o), Some(n))
                };

                // Remove the +/- prefix for display
                let content = if line.len() > 1 && (line.starts_with('+') || line.starts_with('-'))
                {
                    &line[1..]
                } else if line.starts_with(' ') && line.len() > 1 {
                    &line[1..]
                } else {
                    line
                };

                hunk.lines.push(DiffLine::with_line_numbers(
                    content, line_type, old_num, new_num,
                ));
            }
        }

        if let Some(hunk) = current_hunk {
            self.hunks.push(hunk);
        }

        self.update_total_lines();
    }

    /// Clear all diffs
    pub fn clear(&mut self) {
        self.hunks.clear();
        self.scroll = 0;
        self.selected_hunk = 0;
        self.total_lines = 0;
    }

    fn update_total_lines(&mut self) {
        self.total_lines = self.hunks.iter().map(|h| h.lines.len()).sum();
    }

    /// Toggle view mode
    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            DiffViewMode::Unified => DiffViewMode::SideBySide,
            DiffViewMode::SideBySide => DiffViewMode::Unified,
        };
    }

    /// Scroll up
    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    /// Scroll down
    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    /// Page up
    pub fn page_up(&mut self, lines: usize) {
        self.scroll = self.scroll.saturating_sub(lines);
    }

    /// Page down
    pub fn page_down(&mut self, lines: usize) {
        self.scroll = self.scroll.saturating_add(lines);
    }

    /// Next hunk
    pub fn next_hunk(&mut self) {
        if self.selected_hunk < self.hunks.len().saturating_sub(1) {
            self.selected_hunk += 1;
            // Scroll to show the hunk
            let mut line_offset = 0;
            for i in 0..self.selected_hunk {
                line_offset += self.hunks[i].lines.len();
            }
            self.scroll = line_offset;
        }
    }

    /// Previous hunk
    pub fn prev_hunk(&mut self) {
        if self.selected_hunk > 0 {
            self.selected_hunk -= 1;
            let mut line_offset = 0;
            for i in 0..self.selected_hunk {
                line_offset += self.hunks[i].lines.len();
            }
            self.scroll = line_offset;
        }
    }

    /// Render the diff viewer
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(5)])
            .split(area);

        // Tabs for view mode
        let tab_titles = vec!["Unified", "Side-by-Side"];
        let tabs = Tabs::new(tab_titles)
            .block(Block::default().borders(Borders::ALL).title("View Mode"))
            .select(match self.view_mode {
                DiffViewMode::Unified => 0,
                DiffViewMode::SideBySide => 1,
            })
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(tabs, chunks[0]);

        // Diff content
        match self.view_mode {
            DiffViewMode::Unified => self.render_unified(frame, chunks[1]),
            DiffViewMode::SideBySide => self.render_side_by_side(frame, chunks[1]),
        }
    }

    fn render_unified(&self, frame: &mut Frame, area: Rect) {
        let lines: Vec<Line> = self
            .hunks
            .iter()
            .enumerate()
            .flat_map(|(hunk_idx, hunk)| {
                hunk.lines.iter().map(move |line| {
                    let (fg_color, bg_color, prefix) = match line.line_type {
                        DiffLineType::Added => {
                            (Color::Rgb(200, 255, 200), Some(Color::Rgb(30, 50, 30)), "+")
                        }
                        DiffLineType::Removed => {
                            (Color::Rgb(255, 200, 200), Some(Color::Rgb(50, 30, 30)), "-")
                        }
                        DiffLineType::Header => (Color::Rgb(129, 212, 250), None, " "),
                        DiffLineType::HunkHeader => {
                            (Color::Rgb(186, 104, 200), Some(Color::Rgb(40, 30, 50)), " ")
                        }
                        DiffLineType::Context => (Color::Rgb(180, 180, 180), None, " "),
                    };

                    // Build line number display
                    let line_nums = match (line.old_line_number, line.new_line_number) {
                        (Some(o), Some(n)) => format!("{:>4} {:>4} ", o, n),
                        (Some(o), None) => format!("{:>4}      ", o),
                        (None, Some(n)) => format!("     {:>4} ", n),
                        (None, None) => "          ".to_string(),
                    };

                    let mut spans = vec![
                        Span::styled(line_nums, Style::default().fg(Color::Rgb(100, 100, 100))),
                        Span::styled(format!("{} ", prefix), Style::default().fg(fg_color)),
                    ];

                    // Highlight selected hunk
                    let content_style = if hunk_idx == self.selected_hunk {
                        Style::default().fg(fg_color).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(fg_color)
                    };

                    let content_style = if let Some(bg) = bg_color {
                        content_style.bg(bg)
                    } else {
                        content_style
                    };

                    spans.push(Span::styled(&line.content, content_style));

                    Line::from(spans)
                })
            })
            .collect();

        let visible_lines = area.height.saturating_sub(2) as usize;
        let max_scroll = self.total_lines.saturating_sub(visible_lines);
        let scroll = self.scroll.min(max_scroll);

        let stats = self.compute_stats();
        let title = format!(
            "📝 Diff: {} files, +{} -{} ({} hunks)",
            self.hunks.len(),
            stats.additions,
            stats.deletions,
            self.hunks.len()
        );

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(96, 125, 139))),
            )
            .scroll((scroll as u16, 0));

        frame.render_widget(para, area);

        // Scrollbar
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let mut scrollbar_state = ScrollbarState::new(self.total_lines).position(scroll);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }

    fn render_side_by_side(&self, frame: &mut Frame, area: Rect) {
        // Split into two columns
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Left side (old)
        let old_lines: Vec<Line> = self
            .hunks
            .iter()
            .flat_map(|hunk| {
                hunk.lines.iter().filter_map(|line| {
                    match line.line_type {
                        DiffLineType::Removed | DiffLineType::Context => {
                            let num = line
                                .old_line_number
                                .map(|n| format!("{:>4} ", n))
                                .unwrap_or_else(|| "     ".to_string());
                            let style = match line.line_type {
                                DiffLineType::Removed => Style::default()
                                    .fg(Color::Rgb(255, 200, 200))
                                    .bg(Color::Rgb(50, 30, 30)),
                                _ => Style::default().fg(Color::Rgb(180, 180, 180)),
                            };
                            Some(Line::from(vec![
                                Span::styled(num, Style::default().fg(Color::Rgb(100, 100, 100))),
                                Span::styled(&line.content, style),
                            ]))
                        }
                        DiffLineType::Added => Some(Line::from("")), // Empty placeholder
                        _ => None,
                    }
                })
            })
            .collect();

        // Right side (new)
        let new_lines: Vec<Line> = self
            .hunks
            .iter()
            .flat_map(|hunk| {
                hunk.lines.iter().filter_map(|line| {
                    match line.line_type {
                        DiffLineType::Added | DiffLineType::Context => {
                            let num = line
                                .new_line_number
                                .map(|n| format!("{:>4} ", n))
                                .unwrap_or_else(|| "     ".to_string());
                            let style = match line.line_type {
                                DiffLineType::Added => Style::default()
                                    .fg(Color::Rgb(200, 255, 200))
                                    .bg(Color::Rgb(30, 50, 30)),
                                _ => Style::default().fg(Color::Rgb(180, 180, 180)),
                            };
                            Some(Line::from(vec![
                                Span::styled(num, Style::default().fg(Color::Rgb(100, 100, 100))),
                                Span::styled(&line.content, style),
                            ]))
                        }
                        DiffLineType::Removed => Some(Line::from("")), // Empty placeholder
                        _ => None,
                    }
                })
            })
            .collect();

        let visible = area.height.saturating_sub(2) as usize;
        let scroll = self.scroll.min(old_lines.len().saturating_sub(visible));

        let old_para = Paragraph::new(old_lines)
            .block(Block::default().title("Old").borders(Borders::ALL))
            .scroll((scroll as u16, 0));
        frame.render_widget(old_para, columns[0]);

        let new_para = Paragraph::new(new_lines)
            .block(Block::default().title("New").borders(Borders::ALL))
            .scroll((scroll as u16, 0));
        frame.render_widget(new_para, columns[1]);
    }

    /// Compute diff statistics
    fn compute_stats(&self) -> DiffStats {
        let mut stats = DiffStats::default();
        for hunk in &self.hunks {
            for line in &hunk.lines {
                match line.line_type {
                    DiffLineType::Added => stats.additions += 1,
                    DiffLineType::Removed => stats.deletions += 1,
                    _ => {}
                }
            }
        }
        stats
    }
}

/// Parse hunk header like "@@ -1,5 +1,7 @@"
fn parse_hunk_header(line: &str) -> Option<(usize, usize, usize, usize)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }

    let old_range = parts.get(1)?;
    let new_range = parts.get(2)?;

    let parse_range = |s: &str| -> Option<(usize, usize)> {
        let s = s.trim_start_matches(['-', '+'].as_ref());
        let parts: Vec<&str> = s.split(',').collect();
        let start = parts.first()?.parse().ok()?;
        let count = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
        Some((start, count))
    };

    let (old_start, old_count) = parse_range(old_range)?;
    let (new_start, new_count) = parse_range(new_range)?;

    Some((old_start, old_count, new_start, new_count))
}

#[derive(Default)]
struct DiffStats {
    additions: usize,
    deletions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_diff() {
        let mut viewer = DiffViewer::new();
        viewer.compute_diff(
            "test.rs",
            "line1\nline2\nline3\n",
            "line1\nmodified\nline3\nnew line\n",
        );

        assert_eq!(viewer.hunks.len(), 1);
        // Should have some added and removed lines
        let stats = viewer.compute_stats();
        assert!(stats.additions > 0);
        assert!(stats.deletions > 0);
    }

    #[test]
    fn test_parse_hunk_header() {
        let result = parse_hunk_header("@@ -1,5 +1,7 @@");
        assert_eq!(result, Some((1, 5, 1, 7)));
    }
}
