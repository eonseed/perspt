//! Historical note: records used to be pipe separated.

pub fn parse_record(input: &str) -> Result<Vec<String>, String> {
    Ok(input.split('|').map(str::to_owned).collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn current_delimiter_is_comma() {
        assert_eq!(super::parse_record("a,b").unwrap(), vec!["a", "b"]);
    }
}
