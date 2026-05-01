pub mod dag;
pub mod decisions;
pub mod energy;
pub mod llm;
pub mod overview;
pub mod sandbox;
pub mod session_detail;

/// Normalize a node/session state string for consistent display comparisons.
/// Uses `NodeState::from_display_str` for canonical parsing, then maps
/// active states to `"running"` for the dashboard UI.
/// Session-level statuses ("PARTIAL", "ABORTED") are handled before node-state parsing.
pub fn normalize_state(s: &str) -> String {
    // Handle session-level statuses that are not node states.
    match s.to_ascii_lowercase().as_str() {
        "partial" => return "partial".to_string(),
        "aborted" => return "aborted".to_string(),
        _ => {}
    }
    let state = perspt_core::NodeState::from_display_str(s);
    if state.is_active() {
        "running".to_string()
    } else {
        state.to_string()
    }
}

const ADJECTIVES: [&str; 32] = [
    "swift", "bold", "calm", "keen", "warm", "cool", "bright", "sharp", "quiet", "vivid", "pale",
    "deep", "light", "dark", "soft", "firm", "quick", "slow", "wild", "tame", "rare", "vast",
    "slim", "wide", "fair", "pure", "rich", "lean", "raw", "dry", "wet", "old",
];

const NOUNS: [&str; 32] = [
    "oak", "elm", "fox", "owl", "bee", "ant", "ray", "gem", "bay", "ash", "ivy", "fir", "yew",
    "cod", "eel", "jay", "hawk", "dove", "lark", "wren", "pike", "carp", "wolf", "bear", "hare",
    "lynx", "crow", "moth", "seal", "swan", "toad", "newt",
];

/// Generate a deterministic human-readable name from a session UUID.
/// e.g. "0c241cef-490c-..." -> "bold-hawk"
pub fn friendly_name(session_id: &str) -> String {
    let hash = session_id
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    let adj = ADJECTIVES[(hash % 32) as usize];
    let noun = NOUNS[((hash >> 8) % 32) as usize];
    format!("{}-{}", adj, noun)
}
