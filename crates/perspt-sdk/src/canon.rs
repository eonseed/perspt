//! Domain-tagged canonical byte encoding (PSP-10 system 23).
//!
//! serde output is not canonical — field order, float formatting, and escape
//! choices may change across versions — so every digest that must replay
//! commits to this fixed, length-prefixed encoding instead. The pattern is
//! the one `GrantPolicy::canonical_bytes` established; these helpers make it
//! reusable for the prompt plane (`perspt-prompt-v1`) and later digest
//! domains without re-deriving the discipline each time.

use sha2::{Digest, Sha256};

/// An in-progress canonical encoding, opened under one domain tag.
///
/// Every value is length-prefixed (u64 big-endian) so no concatenation can
/// alias two encodings; lists are additionally count-prefixed.
#[derive(Debug)]
pub struct CanonicalEncoder {
    out: Vec<u8>,
}

impl CanonicalEncoder {
    /// Open an encoding under a domain tag such as `b"perspt-prompt-v1"`.
    /// The tag itself is the first length-prefixed field.
    pub fn new(domain_tag: &[u8]) -> Self {
        let mut encoder = Self { out: Vec::new() };
        encoder.field(domain_tag);
        encoder
    }

    /// Append one length-prefixed byte field.
    pub fn field(&mut self, bytes: &[u8]) -> &mut Self {
        self.out
            .extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        self.out.extend_from_slice(bytes);
        self
    }

    /// Append one length-prefixed UTF-8 field.
    pub fn text(&mut self, value: &str) -> &mut Self {
        self.field(value.as_bytes())
    }

    /// Append a fixed-width unsigned integer (no length prefix needed).
    pub fn u64(&mut self, value: u64) -> &mut Self {
        self.out.extend_from_slice(&value.to_be_bytes());
        self
    }

    /// Append one boolean byte.
    pub fn bool(&mut self, value: bool) -> &mut Self {
        self.out.push(u8::from(value));
        self
    }

    /// Append a count-prefixed list of text entries.
    pub fn list<I>(&mut self, entries: I) -> &mut Self
    where
        I: ExactSizeIterator,
        I::Item: AsRef<str>,
    {
        self.out
            .extend_from_slice(&(entries.len() as u64).to_be_bytes());
        for entry in entries {
            self.text(entry.as_ref());
        }
        self
    }

    /// The finished canonical bytes.
    pub fn finish(self) -> Vec<u8> {
        self.out
    }

    /// SHA-256 over the finished canonical bytes, hex encoded with a
    /// `sha256:` prefix so a digest names its own algorithm.
    pub fn digest(self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.out);
        let mut hex = String::with_capacity(7 + 64);
        hex.push_str("sha256:");
        for byte in hasher.finalize() {
            hex.push_str(&format!("{byte:02x}"));
        }
        hex
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_is_length_prefixed_and_unambiguous() {
        let mut a = CanonicalEncoder::new(b"tag");
        a.text("ab").text("c");
        let mut b = CanonicalEncoder::new(b"tag");
        b.text("a").text("bc");
        assert_ne!(a.finish(), b.finish(), "field boundaries must not alias");
    }

    #[test]
    fn lists_are_count_prefixed() {
        let mut a = CanonicalEncoder::new(b"tag");
        a.list(["x", "y"].iter());
        let mut b = CanonicalEncoder::new(b"tag");
        b.list(["x"].iter()).list(["y"].iter());
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn digest_is_deterministic_and_self_describing() {
        let mut a = CanonicalEncoder::new(b"perspt-prompt-v1");
        a.text("section").u64(3).bool(true);
        let mut b = CanonicalEncoder::new(b"perspt-prompt-v1");
        b.text("section").u64(3).bool(true);
        let digest = a.digest();
        assert_eq!(digest, b.digest());
        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest.len(), 7 + 64);
    }
}
