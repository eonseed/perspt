pub mod dag;
pub mod decisions;
pub mod energy;
pub mod llm;
pub mod overview;
pub mod sandbox;
pub mod session_detail;

/// Normalize a node/session state string for consistent comparisons.
/// Maps known variants: Completed/COMPLETED/committed/verified → "completed",
/// RUNNING/Running → "running", FAILED/Failed → "failed", etc.
pub fn normalize_state(s: &str) -> String {
    match s.to_ascii_lowercase().as_str() {
        "completed" | "committed" | "verified" | "stable" => "completed".to_string(),
        "running" | "in_progress" | "in-progress" => "running".to_string(),
        "failed" | "error" => "failed".to_string(),
        "escalated" => "escalated".to_string(),
        other => other.to_string(),
    }
}
