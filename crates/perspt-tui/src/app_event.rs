//! App Event - Message bus for TUI events
//!
//! Provides a decoupled event system inspired by Codex CLI's architecture.
//! All events flow through this enum, enabling async handling via tokio::select!

use crate::task_tree::TaskStatus;
use crossterm::event::Event as CrosstermEvent;

/// Application events for the TUI message bus
#[derive(Debug)]
pub enum AppEvent {
    /// Terminal input event (key press, mouse, resize)
    Terminal(CrosstermEvent),

    /// Streaming chunk from LLM
    StreamChunk(String),

    /// Stream completed (EOT received)
    StreamComplete,

    /// Agent state update (for Agent mode)
    AgentUpdate(AgentStateUpdate),

    /// Periodic tick for animations (throbber, cursor blink)
    Tick,

    /// Request to quit the application
    Quit,

    /// Error event
    Error(String),
}

/// Agent state updates for the Agent mode TUI
#[derive(Debug, Clone)]
pub enum AgentStateUpdate {
    /// Task status changed
    TaskStatusChanged { task_id: String, status: TaskStatus },
    /// Energy value updated
    EnergyUpdated(f32),
    /// Log message
    Log(String),
    /// Node completed
    NodeCompleted(String),
    /// Orchestration finished
    Complete,
}

/// Sender type for AppEvents
pub type AppEventSender = tokio::sync::mpsc::UnboundedSender<AppEvent>;

/// Receiver type for AppEvents
pub type AppEventReceiver = tokio::sync::mpsc::UnboundedReceiver<AppEvent>;

/// Create a new AppEvent channel
pub fn create_app_event_channel() -> (AppEventSender, AppEventReceiver) {
    tokio::sync::mpsc::unbounded_channel()
}
