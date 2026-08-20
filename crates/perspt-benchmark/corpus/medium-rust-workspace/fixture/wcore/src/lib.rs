pub mod data;

pub fn normalize(input: &str) -> String {
    let _ = input;
    todo!()
}

#[cfg(test)]
mod tests {
    #[test]
    fn data_table_is_present() {
        assert!(super::data::ENTRIES.len() > 1000);
    }
}
