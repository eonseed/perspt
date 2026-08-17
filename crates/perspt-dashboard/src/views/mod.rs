pub mod backlog;
pub mod dag;
pub mod decisions;
pub mod energy;
pub mod governance;
pub mod overview;
pub mod psp9;
pub mod session_detail;

/// Normalize a session status string for display: the PSP-9 runtime writes
/// `RUNNING_PSP9` / `COMPLETED_PSP9` / `FAILED_PSP9` / `ESCALATED_PSP9`;
/// strip the suffix and lowercase so badges match on stable names.
pub fn normalize_status(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    lower.trim_end_matches("_psp9").to_string()
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
