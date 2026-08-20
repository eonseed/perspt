/// Parse a size like "2k" using binary suffixes.
pub fn parse_size(input: &str) -> Option<u64> {
    let input = input.trim().to_ascii_lowercase();
    let (digits, multiplier) = match input.as_bytes().last().copied() {
        Some(b'k') => (&input[..input.len() - 1], 1_024),
        Some(b'm') => (&input[..input.len() - 1], 1_048_576),
        Some(b'g') => (&input[..input.len() - 1], 1_073_741_824),
        _ => (input.as_str(), 1),
    };
    digits.parse::<u64>().ok()?.checked_mul(multiplier)
}

#[cfg(test)]
mod tests {
    #[test]
    fn kilo_is_1024() {
        assert_eq!(super::parse_size("2k"), Some(2048));
    }
}
