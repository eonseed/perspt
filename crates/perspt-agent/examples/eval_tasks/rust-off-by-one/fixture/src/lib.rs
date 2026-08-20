pub fn answer() -> u32 { 1 }

#[cfg(test)]
mod tests {
    #[test]
    fn answer_is_positive() { assert!(super::answer() > 0); }
}
