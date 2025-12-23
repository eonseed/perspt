//! Review Modal Component
//!
//! Interactive modal for approving/rejecting changes.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Review decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewDecision {
    Approve,
    Reject,
    RequestChanges,
    Skip,
}

#[derive(Default)]
pub struct ReviewModal {
    /// Whether the modal is visible
    pub visible: bool,
    /// Title/summary
    pub title: String,
    /// Description of changes
    pub description: String,
    /// File paths affected
    pub affected_files: Vec<String>,
    /// Selected action (0 = approve, 1 = reject, 2 = request changes)
    pub selected: usize,
}

impl ReviewModal {
    /// Create a new review modal
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the modal with content
    pub fn show(&mut self, title: String, description: String, files: Vec<String>) {
        self.visible = true;
        self.title = title;
        self.description = description;
        self.affected_files = files;
        self.selected = 0;
    }

    /// Hide the modal
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Move selection left
    pub fn select_left(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Move selection right
    pub fn select_right(&mut self) {
        if self.selected < 2 {
            self.selected += 1;
        }
    }

    /// Get the current decision
    pub fn get_decision(&self) -> ReviewDecision {
        match self.selected {
            0 => ReviewDecision::Approve,
            1 => ReviewDecision::Reject,
            _ => ReviewDecision::RequestChanges,
        }
    }

    /// Render the modal
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        // Center the modal
        let modal_area = centered_rect(60, 50, area);

        // Clear the background
        frame.render_widget(Clear, modal_area);

        // Modal layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(5),    // Description + files
                Constraint::Length(3), // Buttons
            ])
            .split(modal_area);

        // Title
        let title = Paragraph::new(format!("📋 {}", self.title))
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::TOP | Borders::LEFT | Borders::RIGHT));
        frame.render_widget(title, chunks[0]);

        // Description and files
        let mut content = vec![
            Line::from(Span::styled(&self.description, Style::default())),
            Line::default(),
            Line::from(Span::styled(
                "Files affected:",
                Style::default().add_modifier(Modifier::BOLD),
            )),
        ];
        for file in &self.affected_files {
            content.push(Line::from(Span::styled(
                format!("  • {}", file),
                Style::default().fg(Color::Yellow),
            )));
        }

        let description = Paragraph::new(content)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::LEFT | Borders::RIGHT));
        frame.render_widget(description, chunks[1]);

        // Buttons
        let buttons = [("✓ Approve", 0), ("✗ Reject", 1), ("📝 Request Changes", 2)];

        let button_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ])
            .split(chunks[2]);

        for (i, (label, _)) in buttons.iter().enumerate() {
            let style = if i == self.selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let btn = Paragraph::new(*label)
                .style(style)
                .block(Block::default().borders(if i == 0 {
                    Borders::BOTTOM | Borders::LEFT
                } else if i == 2 {
                    Borders::BOTTOM | Borders::RIGHT
                } else {
                    Borders::BOTTOM
                }));
            frame.render_widget(btn, button_chunks[i]);
        }
    }
}

/// Helper to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
