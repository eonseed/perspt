//! Generic runtime smoke-probe scheme (PSP-8).
//!
//! Build + unit tests passing does not prove the produced *artifact runs*: a CLI
//! can panic on startup, a library can fail at import, an entrypoint can crash on
//! a code path no unit test exercised. This module is the language-neutral scheme
//! for exercising built artifacts at runtime and turning failures into typed
//! [`ResidualClass::Runtime`] residuals, so they feed the same energy + directed
//! correction loop as compiler/test residuals instead of slipping through.
//!
//! The split mirrors the rest of `perspt-coding`: an adapter *describes* the
//! smoke invocations and *classifies* their output, but the runtime is what
//! actually executes them (it owns process spawning, timeouts, and sandboxing).
//! Each [`crate::lang::LanguageAdapter`] gets default no-op implementations and
//! overrides them for its language, so new language plugins extend the scheme by
//! implementing two methods.

use perspt_sdk::{IndependenceRoute, ResidualClass, ResidualEvent, ResidualSeverity, SensorRef};

/// A single smoke invocation: a shell command run from the workspace root to
/// exercise a built artifact's runtime entrypoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmokeInvocation {
    /// Full shell command, e.g. `cargo run -q -p cli -- --help`.
    pub command: String,
    /// Human-readable description for logs/telemetry.
    pub description: String,
    /// Whether a non-zero exit is itself a failure. Some entrypoints legitimately
    /// exit non-zero (e.g. a usage error on missing args), so those are run with
    /// `false` and only an in-output crash marker counts as a failure.
    pub failure_on_nonzero: bool,
}

impl SmokeInvocation {
    pub fn new(command: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            description: description.into(),
            failure_on_nonzero: true,
        }
    }

    /// Mark this invocation as tolerating a non-zero exit (only crash markers in
    /// the output will flag it).
    pub fn tolerate_nonzero(mut self) -> Self {
        self.failure_on_nonzero = false;
        self
    }
}

/// Detect a genuine runtime crash in combined stdout+stderr, independent of exit
/// code. Returns the offending line when found. Covers the common cross-language
/// crash signatures (Rust panics/aborts, Python tracebacks, native faults).
pub fn crash_marker(output: &str) -> Option<String> {
    const MARKERS: &[&str] = &[
        "panicked at",
        "RUST_BACKTRACE",
        "fatal runtime error",
        "Traceback (most recent call last)",
        "Segmentation fault",
        "core dumped",
        "AddressSanitizer",
        "SIGSEGV",
        "SIGABRT",
        "Aborted (core dumped)",
        "stack overflow",
        "Uncaught",
        "Unhandled",
    ];
    for line in output.lines() {
        let trimmed = line.trim();
        if MARKERS.iter().any(|m| trimmed.contains(m)) {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// Detect a numeric anomaly (NaN / infinity) in an artifact's output — a strong
/// signal of a scientific-computing/ML defect (divergence, divide-by-zero,
/// unnormalized features). Token-based to avoid substring false positives like
/// "banana" (contains "nan") or "info" (contains "inf"). Returns the token found.
pub fn numeric_anomaly(output: &str) -> Option<String> {
    output
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '+')
        .find(|tok| {
            let t = tok.trim_start_matches(['-', '+']).to_ascii_lowercase();
            matches!(t.as_str(), "nan" | "inf" | "infinity")
        })
        .map(|t| t.to_string())
}

/// The sensor that produced a runtime residual. A real process run is a
/// deterministic-tool route (full-weight eligible), not a model critique.
pub fn runtime_sensor() -> SensorRef {
    SensorRef::new("runtime-smoke", IndependenceRoute::DeterministicTool)
}

/// Build a [`ResidualClass::Runtime`] residual for a failed smoke invocation.
pub fn runtime_residual(
    node_id: &str,
    generation: u32,
    summary: impl Into<String>,
) -> Option<ResidualEvent> {
    let mut r = ResidualEvent::new(
        node_id,
        generation,
        ResidualClass::Runtime,
        ResidualSeverity::Error,
        1.0,
        runtime_sensor(),
    )
    .ok()?;
    r.evidence.summary = summary.into();
    Some(r)
}

/// Default classification shared by adapters: a smoke invocation fails when it
/// exits non-zero (and `failure_on_nonzero`) or its output carries a crash
/// marker. Returns at most one residual per invocation.
pub fn default_classify_runtime(
    node_id: &str,
    generation: u32,
    invocation: &SmokeInvocation,
    exit_success: bool,
    output: &str,
) -> Vec<ResidualEvent> {
    if let Some(line) = crash_marker(output) {
        return runtime_residual(
            node_id,
            generation,
            format!("runtime crash in `{}`: {}", invocation.description, line),
        )
        .into_iter()
        .collect();
    }
    if let Some(tok) = numeric_anomaly(output) {
        return runtime_residual(
            node_id,
            generation,
            format!(
                "numeric anomaly ({tok}) in `{}` output — likely divergence, \
                 divide-by-zero, or unnormalized inputs",
                invocation.description
            ),
        )
        .into_iter()
        .collect();
    }
    if invocation.failure_on_nonzero && !exit_success {
        let tail: String = output
            .lines()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        return runtime_residual(
            node_id,
            generation,
            format!(
                "runtime entrypoint `{}` exited with failure: {}",
                invocation.description, tail
            ),
        )
        .into_iter()
        .collect();
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_panic() {
        let out = "thread 'main' panicked at src/main.rs:10:5:\nindex out of bounds";
        assert!(crash_marker(out).unwrap().contains("panicked at"));
    }

    #[test]
    fn detects_python_traceback() {
        let out = "Traceback (most recent call last):\n  File ...\nValueError: bad";
        assert!(crash_marker(out).is_some());
    }

    #[test]
    fn clean_output_has_no_marker() {
        assert!(crash_marker("Usage: cli <COMMAND>\nвсе хорошо").is_none());
    }

    #[test]
    fn nonzero_exit_flagged_when_required() {
        let inv = SmokeInvocation::new("cli run", "cli");
        let r = default_classify_runtime("n1", 0, &inv, false, "Error: boom");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].class, ResidualClass::Runtime);
    }

    #[test]
    fn tolerated_nonzero_without_crash_is_clean() {
        let inv = SmokeInvocation::new("cli", "cli no-args").tolerate_nonzero();
        // Usage error on missing args is expected, not a runtime defect.
        let r = default_classify_runtime("n1", 0, &inv, false, "error: missing --model");
        assert!(r.is_empty());
    }

    #[test]
    fn numeric_anomaly_detects_nan_and_inf_not_substrings() {
        assert_eq!(
            numeric_anomaly("Forecasted Value: NaN").as_deref(),
            Some("NaN")
        );
        assert!(numeric_anomaly("loss = inf after epoch 3").is_some());
        assert!(numeric_anomaly("result: -inf").is_some());
        // Must NOT false-positive on words containing nan/inf.
        assert!(numeric_anomaly("banana split info panel").is_none());
        assert!(numeric_anomaly("Forecasted Value: 36.0").is_none());
    }

    #[test]
    fn numeric_anomaly_flagged_even_on_success_exit() {
        let inv = SmokeInvocation::new("cli", "cli");
        let r = default_classify_runtime("n1", 0, &inv, true, "Forecasted Value: NaN");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].class, ResidualClass::Runtime);
    }

    #[test]
    fn crash_flagged_even_when_tolerating_nonzero() {
        let inv = SmokeInvocation::new("cli", "cli").tolerate_nonzero();
        let r = default_classify_runtime("n1", 0, &inv, false, "thread 'main' panicked at x");
        assert_eq!(r.len(), 1);
    }
}
