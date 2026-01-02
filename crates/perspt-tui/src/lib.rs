//! perspt-tui: Ratatui-based TUI for Perspt
//!
//! Provides both the Chat TUI for interactive conversations and
//! the Agent TUI for SRBN orchestrator monitoring.

pub mod agent_app;
pub mod chat_app;
pub mod dashboard;
pub mod diff_viewer;
pub mod review_modal;
pub mod simple_input;
pub mod task_tree;
pub mod telemetry;
pub mod theme;
pub mod ui;

// Re-exports for convenient access
pub use agent_app::{run_agent_tui, AgentApp};
pub use chat_app::ChatApp;
pub use dashboard::Dashboard;
pub use diff_viewer::DiffViewer;
pub use review_modal::ReviewModal;
pub use task_tree::TaskTree;
pub use telemetry::{
    create_telemetry_channel, EnergyComponents, TelemetryEvent, TelemetryReceiver, TelemetrySender,
};
pub use theme::Theme;
pub use ui::{run_chat_tui, AppMode};

// Legacy re-export
#[allow(deprecated)]
pub use ui::run_tui;
