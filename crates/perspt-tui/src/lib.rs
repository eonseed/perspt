//! perspt-tui: Ratatui-based TUI for Perspt
//!
//! Provides both the legacy Chat TUI and the new Agent TUI.

pub mod agent_app;
pub mod dashboard;
pub mod diff_viewer;
pub mod review_modal;
pub mod task_tree;
pub mod ui;

pub use agent_app::{run_agent_tui, AgentApp};
pub use dashboard::Dashboard;
pub use diff_viewer::DiffViewer;
pub use review_modal::ReviewModal;
pub use task_tree::TaskTree;
pub use ui::run_tui;
