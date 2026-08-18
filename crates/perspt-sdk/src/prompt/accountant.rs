//! Versioned token accounting (Definition 4's size function; Definition 6's
//! accountant `c`).
//!
//! v1 is a conservative upper bound, not an exact tokenizer: BPE English
//! and code average roughly four bytes per token, so `ceil(bytes / 3)`
//! overestimates by a comfortable margin, and Definition 6's guard reserve
//! absorbs residual error. An underestimating accountant is not valid. The
//! accountant's identity and mode enter every program digest, so a later
//! exact tokenizer is a clean versioned swap with no silent replay
//! divergence.

use serde::{Deserialize, Serialize};

/// Whether counts are exact or a declared upper bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountingMode {
    ExactTokenizer,
    ConservativeUpperBound,
}

/// A named, versioned token accounting function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenAccountantRef {
    /// e.g. `"approx_bytes_v1"`.
    pub id: String,
    pub mode: AccountingMode,
}

/// Fixed per-message framing overhead charged by v1 (role tags, separators,
/// provider envelope slack).
const MESSAGE_FRAMING_TOKENS: u64 = 8;

impl TokenAccountantRef {
    /// The v1 conservative accountant used until an exact tokenizer ships
    /// for a route.
    pub fn approx_bytes_v1() -> Self {
        Self {
            id: "approx_bytes_v1".into(),
            mode: AccountingMode::ConservativeUpperBound,
        }
    }

    /// Upper-bound token count of one text.
    pub fn count_text(&self, text: &str) -> u64 {
        (text.len() as u64).div_ceil(3)
    }

    /// Upper-bound token count of one message (content plus framing).
    pub fn count_message(&self, content: &str) -> u64 {
        self.count_text(content) + MESSAGE_FRAMING_TOKENS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counting_is_a_conservative_upper_bound() {
        let accountant = TokenAccountantRef::approx_bytes_v1();
        assert_eq!(accountant.count_text(""), 0);
        assert_eq!(accountant.count_text("abc"), 1);
        assert_eq!(accountant.count_text("abcd"), 2);
        // ~4 bytes/token real-world average is always under bytes/3.
        let text = "let value = compute(input); // typical code density";
        assert!(accountant.count_text(text) * 4 > text.len() as u64);
        assert_eq!(accountant.count_message("abc"), 1 + MESSAGE_FRAMING_TOKENS);
    }
}
