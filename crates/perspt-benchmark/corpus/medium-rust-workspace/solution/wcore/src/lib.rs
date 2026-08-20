pub mod data;

pub fn normalize(input: &str) -> String {
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    #[test]
    fn data_table_is_present() {
        assert!(super::data::ENTRIES.len() > 1000);
    }
}
