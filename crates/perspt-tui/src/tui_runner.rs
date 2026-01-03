//! TUI Runner - Async event loop for responsive TUI
//!
//! Provides a centralized async event loop using tokio::select! that handles:
//! - Terminal events (keyboard, mouse, resize) via crossterm EventStream
//! - Backend events (streaming chunks, agent updates) via channels
//! - Periodic ticks for animations
//!
//! Inspired by Codex CLI's architecture for maximum responsiveness.

use crate::app_event::{AppEvent, AppEventReceiver, AppEventSender};
use crossterm::event::EventStream;
use futures::StreamExt;
use std::io::{self, stdout, Stdout};
use std::time::Duration;
use tokio::time::interval;

use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

/// Terminal type alias
pub type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

/// Frame rate for animation ticks (60 FPS)
const TICK_RATE_MS: u64 = 16;

/// Minimum time between renders to avoid excessive CPU usage
const MIN_RENDER_INTERVAL_MS: u64 = 16;

/// TUI Runner configuration
pub struct TuiRunnerConfig {
    /// Enable mouse capture
    pub mouse_capture: bool,
    /// Enable keyboard enhancement flags
    pub keyboard_enhancement: bool,
    /// Use alternate screen
    pub alternate_screen: bool,
}

impl Default for TuiRunnerConfig {
    fn default() -> Self {
        Self {
            mouse_capture: true,
            keyboard_enhancement: true,
            alternate_screen: false, // Inline mode by default for chat
        }
    }
}

/// Initialize terminal with optional settings
pub fn init_terminal(config: &TuiRunnerConfig) -> io::Result<TuiTerminal> {
    enable_raw_mode()?;

    if config.alternate_screen {
        execute!(stdout(), EnterAlternateScreen)?;
    }

    // Enable keyboard enhancement for better modifier detection
    if config.keyboard_enhancement {
        let _ = execute!(
            stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        );
    }

    execute!(stdout(), EnableBracketedPaste)?;

    if config.mouse_capture {
        execute!(stdout(), EnableMouseCapture)?;
    }

    let backend = CrosstermBackend::new(stdout());
    Terminal::new(backend)
}

/// Restore terminal to original state
pub fn restore_terminal(config: &TuiRunnerConfig) -> io::Result<()> {
    if config.keyboard_enhancement {
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    }

    if config.mouse_capture {
        let _ = execute!(stdout(), DisableMouseCapture);
    }

    execute!(stdout(), DisableBracketedPaste)?;

    if config.alternate_screen {
        execute!(stdout(), LeaveAlternateScreen)?;
    }

    disable_raw_mode()?;
    Ok(())
}

/// Run the async event loop
///
/// This function drives the TUI by:
/// 1. Listening for terminal events via EventStream
/// 2. Listening for app events via the receiver channel
/// 3. Sending periodic tick events for animations
///
/// All events are forwarded to the app_tx channel for unified handling.
pub async fn run_event_loop(
    app_tx: AppEventSender,
    mut app_rx: AppEventReceiver,
    mut on_event: impl FnMut(AppEvent) -> bool, // Returns false to quit
) {
    let mut event_stream = EventStream::new();
    let mut tick_interval = interval(Duration::from_millis(TICK_RATE_MS));

    loop {
        tokio::select! {
            // Terminal events (keyboard, mouse, resize)
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(event)) => {
                        if !on_event(AppEvent::Terminal(event)) {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        let _ = app_tx.send(AppEvent::Error(e.to_string()));
                    }
                    None => break, // Stream ended
                }
            }

            // App events from other sources (streaming, agent updates)
            Some(event) = app_rx.recv() => {
                if !on_event(event) {
                    break;
                }
            }

            // Periodic tick for animations
            _ = tick_interval.tick() => {
                if !on_event(AppEvent::Tick) {
                    break;
                }
            }
        }
    }
}

/// Frame rate limiter to prevent excessive rendering
pub struct FrameRateLimiter {
    last_render: std::time::Instant,
    min_interval: Duration,
}

impl Default for FrameRateLimiter {
    fn default() -> Self {
        Self {
            last_render: std::time::Instant::now(),
            min_interval: Duration::from_millis(MIN_RENDER_INTERVAL_MS),
        }
    }
}

impl FrameRateLimiter {
    /// Check if enough time has passed for a new render
    pub fn should_render(&mut self) -> bool {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_render) >= self.min_interval {
            self.last_render = now;
            true
        } else {
            false
        }
    }

    /// Force a render (for important updates)
    pub fn force_render(&mut self) {
        self.last_render = std::time::Instant::now();
    }
}
