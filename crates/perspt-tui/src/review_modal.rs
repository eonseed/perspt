//! Review Modal Component
//!
//! Interactive modal for approving/rejecting agent-proposed changes
//! with stability metrics display and keyboard shortcuts.

use crate::telemetry::EnergyComponents;
use crate::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Paragraph, Wrap},
    Frame,
};

/// Review decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewDecision {
    /// Approve changes [y]
    Approve,
    /// Reject changes [n]
    Reject,
    /// Edit in external editor [e]
    Edit,
    /// View detailed diff [d]
    ViewDiff,
    /// Skip this review
    Skip,
}

impl ReviewDecision {
    pub fn hotkey(&self) -> char {
        match self {
            ReviewDecision::Approve => 'y',
            ReviewDecision::Reject => 'n',
            ReviewDecision::Edit => 'e',
            ReviewDecision::ViewDiff => 'd',
            ReviewDecision::Skip => 's',
        }
    }
}

/// Stability metrics for display
#[derive(Debug, Clone, Default)]
pub struct StabilityMetrics {
    /// Lyapunov energy components
    pub energy: EnergyComponents,
    /// Whether system is stable (V(x) < ε)
    pub is_stable: bool,
    /// Stability threshold (ε)
    pub threshold: f32,
    /// Number of convergence attempts
    pub attempts: usize,
    /// Maximum allowed attempts
    pub max_attempts: usize,
}

/// Enhanced review modal with stability metrics
pub struct ReviewModal {
    /// Whether the modal is visible
    pub visible: bool,
    /// Title/summary
    pub title: String,
    /// Description of changes
    pub description: String,
    /// File paths affected
    pub affected_files: Vec<String>,
    /// Selected action index
    pub selected: usize,
    /// Stability metrics
    pub stability: Option<StabilityMetrics>,
    /// Theme for styling
    theme: Theme,
    /// Available actions
    actions: Vec<(ReviewDecision, &'static str, &'static str)>,
}

impl Default for ReviewModal {
    fn default() -> Self {
        Self {
            visible: false,
            title: String::new(),
            description: String::new(),
            affected_files: Vec::new(),
            selected: 0,
            stability: None,
            theme: Theme::default(),
            actions: vec![
                (ReviewDecision::Approve, "y", "✓ Approve"),
                (ReviewDecision::Reject, "n", "✗ Reject"),
                (ReviewDecision::Edit, "e", "📝 Edit"),
                (ReviewDecision::ViewDiff, "d", "👁 Diff"),
            ],
        }
    }
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
        self.stability = None;
    }

    /// Show the modal with stability metrics
    pub fn show_with_stability(
        &mut self,
        title: String,
        description: String,
        files: Vec<String>,
        stability: StabilityMetrics,
    ) {
        self.show(title, description, files);
        self.stability = Some(stability);
    }

    /// Hide the modal
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Handle keyboard input, returns decision if action taken
    pub fn handle_key(&mut self, key: char) -> Option<ReviewDecision> {
        match key.to_ascii_lowercase() {
            'y' => Some(ReviewDecision::Approve),
            'n' => Some(ReviewDecision::Reject),
            'e' => Some(ReviewDecision::Edit),
            'd' => Some(ReviewDecision::ViewDiff),
            's' => Some(ReviewDecision::Skip),
            _ => None,
        }
    }

    /// Move selection left
    pub fn select_left(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Move selection right
    pub fn select_right(&mut self) {
        if self.selected < self.actions.len() - 1 {
            self.selected += 1;
        }
    }

    /// Get the current decision
    pub fn get_decision(&self) -> ReviewDecision {
        self.actions
            .get(self.selected)
            .map(|(d, _, _)| *d)
            .unwrap_or(ReviewDecision::Skip)
    }

    /// Render the modal
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        // Determine modal size based on whether we have stability metrics
        let height_percent = if self.stability.is_some() { 65 } else { 50 };
        let modal_area = centered_rect(65, height_percent, area);

        // Clear the background
        frame.render_widget(Clear, modal_area);

        // Modal background
        let bg_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(96, 125, 139)))
            .style(Style::default().bg(Color::Rgb(30, 30, 35)));
        frame.render_widget(bg_block, modal_area);

        // Inner content area
        let inner = modal_area.inner(ratatui::layout::Margin {
            vertical: 1,
            horizontal: 1,
        });

        // Layout depends on whether we have stability metrics
        let constraints = if self.stability.is_some() {
            vec![
                Constraint::Length(2), // Title
                Constraint::Length(6), // Stability metrics
                Constraint::Min(4),    // Description + files
                Constraint::Length(3), // Buttons
                Constraint::Length(1), // Keyboard hints
            ]
        } else {
            vec![
                Constraint::Length(2), // Title
                Constraint::Min(6),    // Description + files
                Constraint::Length(3), // Buttons
                Constraint::Length(1), // Keyboard hints
            ]
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner);

        let mut chunk_idx = 0;

        // Title
        let title = Paragraph::new(format!("📋 {}", self.title)).style(
            Style::default()
                .fg(Color::Rgb(129, 212, 250))
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(title, chunks[chunk_idx]);
        chunk_idx += 1;

        // Stability metrics (if available)
        if let Some(ref stability) = self.stability {
            self.render_stability_metrics(frame, chunks[chunk_idx], stability);
            chunk_idx += 1;
        }

        // Description and files
        self.render_description(frame, chunks[chunk_idx]);
        chunk_idx += 1;

        // Buttons
        self.render_buttons(frame, chunks[chunk_idx]);
        chunk_idx += 1;

        // Keyboard hints
        let hints = Paragraph::new(Line::from(vec![
            Span::styled("Shortcuts: ", Style::default().fg(Color::DarkGray)),
            Span::styled("[y]", Style::default().fg(Color::Green)),
            Span::raw(" approve  "),
            Span::styled("[n]", Style::default().fg(Color::Red)),
            Span::raw(" reject  "),
            Span::styled("[e]", Style::default().fg(Color::Yellow)),
            Span::raw(" edit  "),
            Span::styled("[d]", Style::default().fg(Color::Cyan)),
            Span::raw(" diff  "),
            Span::styled("[Esc]", Style::default().fg(Color::DarkGray)),
            Span::raw(" cancel"),
        ]))
        .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hints, chunks[chunk_idx]);
    }

    fn render_stability_metrics(
        &self,
        frame: &mut Frame,
        area: Rect,
        stability: &StabilityMetrics,
    ) {
        let energy = &stability.energy;

        // Split into columns for energy components
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25), // Total energy
                Constraint::Percentage(25), // V_syn
                Constraint::Percentage(25), // V_str
                Constraint::Percentage(25), // V_log
            ])
            .split(area);

        // Total energy with status
        let (status_text, status_color) = if stability.is_stable {
            ("✓ STABLE", Color::Rgb(102, 187, 106))
        } else {
            ("⚡ CONVERGING", Color::Rgb(255, 183, 77))
        };

        let energy_style = self.theme.energy_style(energy.total);
        let total_block = Block::default()
            .title(Span::styled(status_text, Style::default().fg(status_color)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(status_color));

        let total_gauge = Gauge::default()
            .block(total_block)
            .gauge_style(energy_style)
            .ratio((energy.total.min(1.0)) as f64)
            .label(format!("V(x)={:.3}", energy.total));
        frame.render_widget(total_gauge, cols[0]);

        // Component gauges
        let components = [
            ("V_syn", energy.v_syn, Color::Rgb(129, 212, 250)),
            ("V_str", energy.v_str, Color::Rgb(186, 104, 200)),
            ("V_log", energy.v_log, Color::Rgb(255, 183, 77)),
        ];

        for (i, (name, value, color)) in components.iter().enumerate() {
            let gauge = Gauge::default()
                .block(Block::default().title(*name).borders(Borders::ALL))
                .gauge_style(Style::default().fg(*color))
                .ratio((*value as f64).min(1.0))
                .label(format!("{:.2}", value));
            frame.render_widget(gauge, cols[i + 1]);
        }
    }

    fn render_description(&self, frame: &mut Frame, area: Rect) {
        let mut content = vec![
            Line::from(Span::styled(
                &self.description,
                Style::default().fg(Color::White),
            )),
            Line::default(),
        ];

        if !self.affected_files.is_empty() {
            content.push(Line::from(Span::styled(
                format!("Files affected ({}):", self.affected_files.len()),
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::White),
            )));

            for file in self.affected_files.iter().take(5) {
                content.push(Line::from(vec![
                    Span::styled("  📄 ", Style::default()),
                    Span::styled(file, Style::default().fg(Color::Rgb(255, 183, 77))),
                ]));
            }

            if self.affected_files.len() > 5 {
                content.push(Line::from(Span::styled(
                    format!("  ... and {} more", self.affected_files.len() - 5),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        let description = Paragraph::new(content).wrap(Wrap { trim: true });
        frame.render_widget(description, area);
    }

    fn render_buttons(&self, frame: &mut Frame, area: Rect) {
        let constraints: Vec<Constraint> = self
            .actions
            .iter()
            .map(|_| Constraint::Ratio(1, self.actions.len() as u32))
            .collect();

        let button_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        for (i, (decision, key, label)) in self.actions.iter().enumerate() {
            let is_selected = i == self.selected;

            let (fg, bg) = if is_selected {
                (
                    Color::Black,
                    match decision {
                        ReviewDecision::Approve => Color::Rgb(102, 187, 106),
                        ReviewDecision::Reject => Color::Rgb(239, 83, 80),
                        ReviewDecision::Edit => Color::Rgb(255, 183, 77),
                        ReviewDecision::ViewDiff => Color::Rgb(129, 212, 250),
                        ReviewDecision::Skip => Color::DarkGray,
                    },
                )
            } else {
                (Color::White, Color::Rgb(50, 50, 55))
            };

            let btn_text = format!("[{}] {}", key, label);
            let btn = Paragraph::new(btn_text)
                .style(Style::default().fg(fg).bg(bg).add_modifier(if is_selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }))
                .alignment(ratatui::layout::HorizontalAlignment::Center)
                .block(Block::default().borders(Borders::ALL).border_style(
                    Style::default().fg(if is_selected { bg } else { Color::DarkGray }),
                ));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_shortcuts() {
        let mut modal = ReviewModal::new();
        modal.show("Test".to_string(), "Desc".to_string(), vec![]);

        assert_eq!(modal.handle_key('y'), Some(ReviewDecision::Approve));
        assert_eq!(modal.handle_key('n'), Some(ReviewDecision::Reject));
        assert_eq!(modal.handle_key('e'), Some(ReviewDecision::Edit));
        assert_eq!(modal.handle_key('d'), Some(ReviewDecision::ViewDiff));
        assert_eq!(modal.handle_key('x'), None);
    }

    #[test]
    fn test_navigation() {
        let mut modal = ReviewModal::new();
        modal.show("Test".to_string(), "Desc".to_string(), vec![]);

        assert_eq!(modal.selected, 0);
        modal.select_right();
        assert_eq!(modal.selected, 1);
        modal.select_left();
        assert_eq!(modal.selected, 0);
    }
}
