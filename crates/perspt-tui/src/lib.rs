//! perspt-tui: Ratatui-based TUI for Perspt
//!
//! Provides both the Chat TUI for interactive conversations and
//! the Agent TUI for SRBN orchestrator monitoring.

pub mod agent_app;
pub mod app_event;
pub mod chat_app;
mod chat_commands;
mod chat_view;
pub mod dashboard;
pub mod diff_viewer;
pub mod latex;
pub mod markdown;
pub mod review_modal;
pub mod simple_input;
pub mod task_tree;
pub mod telemetry;
pub mod theme;
pub mod tui_runner;
pub mod ui;

// Re-exports for convenient access
pub use agent_app::{run_agent_tui_with_runtime, AgentApp};
pub use app_event::{create_app_event_channel, AppEvent, AppEventReceiver, AppEventSender};
pub use chat_app::ChatApp;
pub use dashboard::Dashboard;
pub use diff_viewer::DiffViewer;
pub use review_modal::ReviewModal;
pub use telemetry::EnergyComponents;
pub use theme::Theme;
pub use ui::run_chat_tui;
