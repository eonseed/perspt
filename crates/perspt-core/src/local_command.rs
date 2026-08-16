//! Provider-independent commands handled entirely by Perspt frontends.

/// A local command that must never be forwarded to a model provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalCommand {
    /// Display Perspt's dedication.
    Love,
}

pub const DEDICATION_TITLE: &str = "Special Dedication";
pub const DEDICATION_PREAMBLE: &str = "This application is lovingly dedicated to";
pub const DEDICATION_FAMILY: &str =
    "my wonderful mothers DaiJun(黛君), VijayLaxmi and grandma Sushila";
pub const DEDICATION_THANKS: &str = "Thank you for your endless love, wisdom, and support";
pub const DEDICATION_CLOSING: &str = "With all my love and gratitude";

/// Parse a command that is deliberately handled without an LLM round trip.
pub fn parse_local_command(input: &str) -> Option<LocalCommand> {
    input
        .trim()
        .eq_ignore_ascii_case("l-o-v-e")
        .then_some(LocalCommand::Love)
}

/// Plain-text rendering shared by non-ANSI frontends.
pub fn dedication_text() -> String {
    format!(
        "{DEDICATION_TITLE}\n\n{DEDICATION_PREAMBLE}\n   {DEDICATION_FAMILY}\n\n\
         {DEDICATION_THANKS}\n\n{DEDICATION_CLOSING}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn love_is_case_insensitive_and_exact() {
        assert_eq!(parse_local_command("l-o-v-e"), Some(LocalCommand::Love));
        assert_eq!(parse_local_command("  L-O-V-E  "), Some(LocalCommand::Love));
        assert_eq!(parse_local_command("love"), None);
        assert_eq!(parse_local_command("l-o-v-e now"), None);
    }

    #[test]
    fn dedication_names_the_family() {
        let text = dedication_text();
        assert!(text.contains("DaiJun(黛君), VijayLaxmi and grandma Sushila"));
    }
}
