//! Theme module for consistent styling across the TUI
//!
//! Provides semantic colors optimized for dark terminal backgrounds.

use ratatui::style::{Color, Modifier, Style};

/// Theme configuration for the TUI
#[derive(Debug, Clone)]
pub struct Theme {
    /// User message styling
    pub user_message: Style,
    /// Assistant message styling
    pub assistant_message: Style,
    /// System/info message styling
    pub system_message: Style,
    /// Code block background
    pub code_block: Style,
    /// Success/stable state
    pub success: Style,
    /// Warning state
    pub warning: Style,
    /// Error/failure state
    pub error: Style,
    /// High energy (unstable)
    pub energy_high: Style,
    /// Medium energy (converging)
    pub energy_medium: Style,
    /// Low energy (stable)
    pub energy_low: Style,
    /// Border styling
    pub border: Style,
    /// Highlight/selected item
    pub highlight: Style,
    /// Muted/secondary text
    pub muted: Style,
    /// Streaming cursor
    pub cursor: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Dark theme optimized for modern terminals (Ghostty, iTerm2, etc.)
    pub fn dark() -> Self {
        Self {
            // Messages
            user_message: Style::default()
                .fg(Color::Rgb(129, 199, 132)) // Soft green
                .add_modifier(Modifier::BOLD),
            assistant_message: Style::default().fg(Color::Rgb(144, 202, 249)), // Soft blue
            system_message: Style::default().fg(Color::Rgb(176, 190, 197)),    // Blue-gray

            // Code
            code_block: Style::default()
                .fg(Color::Rgb(248, 248, 242)) // Off-white
                .bg(Color::Rgb(40, 42, 54)), // Dark background

            // Status
            success: Style::default().fg(Color::Rgb(102, 187, 106)), // Green
            warning: Style::default().fg(Color::Rgb(255, 183, 77)),  // Amber
            error: Style::default().fg(Color::Rgb(239, 83, 80)),     // Red

            // Energy levels (Lyapunov)
            energy_high: Style::default()
                .fg(Color::Rgb(239, 83, 80)) // Red - unstable
                .add_modifier(Modifier::BOLD),
            energy_medium: Style::default().fg(Color::Rgb(255, 183, 77)), // Amber - converging
            energy_low: Style::default()
                .fg(Color::Rgb(102, 187, 106)) // Green - stable
                .add_modifier(Modifier::BOLD),

            // UI elements
            border: Style::default().fg(Color::Rgb(96, 125, 139)), // Blue-gray
            highlight: Style::default()
                .fg(Color::Rgb(224, 247, 250)) // Cyan tint
                .bg(Color::Rgb(55, 71, 79)) // Dark selection
                .add_modifier(Modifier::BOLD),
            muted: Style::default().fg(Color::Rgb(120, 144, 156)), // Dim gray

            // Cursor for streaming
            cursor: Style::default()
                .fg(Color::Rgb(129, 212, 250)) // Light cyan
                .add_modifier(Modifier::SLOW_BLINK),
        }
    }

    /// Get style for energy value (Lyapunov)
    pub fn energy_style(&self, energy: f32) -> Style {
        if energy < 0.1 {
            self.energy_low
        } else if energy < 0.5 {
            self.energy_medium
        } else {
            self.energy_high
        }
    }

    /// Get style for task status
    pub fn status_style(&self, status: &str) -> Style {
        match status.to_lowercase().as_str() {
            "completed" | "stable" | "ok" => self.success,
            "running" | "pending" | "converging" => self.warning,
            "failed" | "error" | "escalated" => self.error,
            _ => self.muted,
        }
    }
}

/// Unicode icons for terminal display
pub mod icons {
    pub const USER: &str = "🧑";
    pub const ASSISTANT: &str = "🤖";
    pub const SYSTEM: &str = "ℹ️";
    pub const SUCCESS: &str = "✓";
    pub const FAILURE: &str = "✗";
    pub const WARNING: &str = "⚠";
    pub const PENDING: &str = "○";
    pub const RUNNING: &str = "◐";
    pub const COMPLETED: &str = "●";
    pub const ROCKET: &str = "🚀";
    pub const TREE: &str = "🌳";
    pub const FILE: &str = "📄";
    pub const FOLDER: &str = "📁";
    pub const ENERGY: &str = "⚡";
    pub const STABLE: &str = "🔒";
    pub const CURSOR: &str = "▌";
}
