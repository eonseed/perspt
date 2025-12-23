//! Agent Dashboard Component
//!
//! Main dashboard view for the Agent TUI showing task progress, energy, and status.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline, Tabs},
    Frame,
};

/// Dashboard state
pub struct Dashboard {
    /// Current node being executed
    pub current_node: Option<String>,
    /// Total nodes in the task
    pub total_nodes: usize,
    /// Completed nodes
    pub completed_nodes: usize,
    /// Current Lyapunov Energy
    pub energy: f32,
    /// Energy history for sparkline
    pub energy_history: Vec<u64>,
    /// Stability status
    pub stable: bool,
    /// Current status message
    pub status: String,
    /// Log messages
    pub logs: Vec<String>,
}

impl Default for Dashboard {
    fn default() -> Self {
        Self {
            current_node: None,
            total_nodes: 0,
            completed_nodes: 0,
            energy: 0.0,
            energy_history: Vec::new(),
            stable: false,
            status: "Ready".to_string(),
            logs: Vec::new(),
        }
    }
}

impl Dashboard {
    /// Create a new dashboard
    pub fn new() -> Self {
        Self::default()
    }

    /// Update energy and push to history
    pub fn update_energy(&mut self, energy: f32) {
        self.energy = energy;
        // Convert to u64 for sparkline (scale to 0-100)
        let scaled = ((energy * 100.0).clamp(0.0, 100.0)) as u64;
        self.energy_history.push(scaled);
        // Keep only last 50 values
        if self.energy_history.len() > 50 {
            self.energy_history.remove(0);
        }
        self.stable = energy < 0.1; // epsilon threshold
    }

    /// Add a log message
    pub fn log(&mut self, message: String) {
        self.logs.push(message);
        // Keep only last 100 logs
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    /// Render the dashboard
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Main layout: Header, Content (2 columns), Footer
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Content
                Constraint::Length(3), // Footer
            ])
            .split(area);

        self.render_header(frame, main_chunks[0]);
        self.render_content(frame, main_chunks[1]);
        self.render_footer(frame, main_chunks[2]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let header_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ])
            .split(area);

        // Title
        let title = Paragraph::new("🚀 SRBN Agent Dashboard")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, header_chunks[0]);

        // Progress
        let progress_ratio = if self.total_nodes > 0 {
            self.completed_nodes as f64 / self.total_nodes as f64
        } else {
            0.0
        };
        let progress_label = format!("{}/{} nodes", self.completed_nodes, self.total_nodes);
        let progress = Gauge::default()
            .block(Block::default().title("Progress").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Green))
            .ratio(progress_ratio)
            .label(progress_label);
        frame.render_widget(progress, header_chunks[1]);

        // Stability indicator
        let (stability_text, stability_color) = if self.stable {
            ("✓ STABLE", Color::Green)
        } else {
            ("⚡ CONVERGING", Color::Yellow)
        };
        let stability = Paragraph::new(stability_text)
            .style(
                Style::default()
                    .fg(stability_color)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().title("Status").borders(Borders::ALL));
        frame.render_widget(stability, header_chunks[2]);
    }

    fn render_content(&self, frame: &mut Frame, area: Rect) {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Left: Energy and current node
        self.render_energy_panel(frame, content_chunks[0]);

        // Right: Logs
        self.render_log_panel(frame, content_chunks[1]);
    }

    fn render_energy_panel(&self, frame: &mut Frame, area: Rect) {
        let panel_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Current node
                Constraint::Length(3), // Energy value
                Constraint::Min(5),    // Energy sparkline
            ])
            .split(area);

        // Current node
        let node_text = self.current_node.as_deref().unwrap_or("None");
        let node = Paragraph::new(format!("📋 {}", node_text))
            .block(Block::default().title("Current Task").borders(Borders::ALL));
        frame.render_widget(node, panel_chunks[0]);

        // Energy value
        let energy_color = if self.energy < 0.1 {
            Color::Green
        } else if self.energy < 0.5 {
            Color::Yellow
        } else {
            Color::Red
        };
        let energy = Paragraph::new(format!("V(x) = {:.4}", self.energy))
            .style(
                Style::default()
                    .fg(energy_color)
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .title("Lyapunov Energy")
                    .borders(Borders::ALL),
            );
        frame.render_widget(energy, panel_chunks[1]);

        // Energy sparkline
        let sparkline = Sparkline::default()
            .block(
                Block::default()
                    .title("Energy History")
                    .borders(Borders::ALL),
            )
            .data(&self.energy_history)
            .style(Style::default().fg(Color::Magenta));
        frame.render_widget(sparkline, panel_chunks[2]);
    }

    fn render_log_panel(&self, frame: &mut Frame, area: Rect) {
        let log_items: Vec<ListItem> = self
            .logs
            .iter()
            .rev()
            .take(20)
            .map(|log| {
                let style = if log.contains("ERROR") {
                    Style::default().fg(Color::Red)
                } else if log.contains("WARN") {
                    Style::default().fg(Color::Yellow)
                } else if log.contains("OK") || log.contains("STABLE") {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };
                ListItem::new(log.as_str()).style(style)
            })
            .collect();

        let logs = List::new(log_items).block(
            Block::default()
                .title("📜 Activity Log")
                .borders(Borders::ALL),
        );
        frame.render_widget(logs, area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let help = Paragraph::new(
            "Press 'q' to quit | 'p' to pause | 'r' to resume | 'a' to approve | 'd' to reject",
        )
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(help, area);
    }
}
