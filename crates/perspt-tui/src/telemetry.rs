//! Telemetry module for real-time orchestrator updates
//!
//! Provides event types and channels for dashboard communication.

use tokio::sync::mpsc;

/// Telemetry event types from the orchestrator
#[derive(Debug, Clone)]
pub enum TelemetryEvent {
    /// Lyapunov Energy update for a node
    EnergyUpdate {
        node_id: String,
        energy: f32,
        components: EnergyComponents,
    },
    /// Task status change
    TaskStatusChange {
        node_id: String,
        status: TaskStatus,
        goal: String,
    },
    /// Token usage update
    TokensUsed { count: usize, total: usize },
    /// Log message for activity feed
    Log(LogEntry),
    /// Session state change
    SessionState(SessionState),
}

/// Energy components for detailed display
#[derive(Debug, Clone, Default)]
pub struct EnergyComponents {
    /// Syntactic energy (LSP diagnostics)
    pub v_syn: f32,
    /// Structural energy (contract violations)
    pub v_str: f32,
    /// Logic energy (test failures)
    pub v_log: f32,
    /// Total composite energy
    pub total: f32,
}

impl EnergyComponents {
    pub fn new(v_syn: f32, v_str: f32, v_log: f32, alpha: f32, beta: f32, gamma: f32) -> Self {
        Self {
            v_syn,
            v_str,
            v_log,
            total: alpha * v_syn + beta * v_str + gamma * v_log,
        }
    }

    pub fn is_stable(&self, epsilon: f32) -> bool {
        self.total < epsilon
    }
}

/// Task status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Verifying,
    Completed,
    Failed,
    Escalated,
}

impl TaskStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "○",
            TaskStatus::Running => "◐",
            TaskStatus::Verifying => "◑",
            TaskStatus::Completed => "●",
            TaskStatus::Failed => "✗",
            TaskStatus::Escalated => "⚠",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Verifying => "verifying",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Escalated => "escalated",
        }
    }
}

/// Log entry for activity feed
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: std::time::Instant,
}

impl LogEntry {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            level: LogLevel::Info,
            message: message.into(),
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            level: LogLevel::Warning,
            message: message.into(),
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: LogLevel::Error,
            message: message.into(),
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self {
            level: LogLevel::Success,
            message: message.into(),
            timestamp: std::time::Instant::now(),
        }
    }
}

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Success,
}

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Planning,
    Executing,
    Paused,
    Completed,
    Failed,
}

/// Type alias for telemetry sender
pub type TelemetrySender = mpsc::UnboundedSender<TelemetryEvent>;

/// Type alias for telemetry receiver
pub type TelemetryReceiver = mpsc::UnboundedReceiver<TelemetryEvent>;

/// Create a new telemetry channel
pub fn create_telemetry_channel() -> (TelemetrySender, TelemetryReceiver) {
    mpsc::unbounded_channel()
}
