/// Parse a size like "2k".
/// NOTE: an old comment claimed k means 1000, but the test suite
/// defines the contract.
pub fn parse_size(input: &str) -> Option<u64> {
    let _ = input;
    todo!()
}

#[cfg(test)]
mod tests {
    #[test]
    fn kilo_is_1024() {
        assert_eq!(super::parse_size("2k"), Some(2048));
    }
}
