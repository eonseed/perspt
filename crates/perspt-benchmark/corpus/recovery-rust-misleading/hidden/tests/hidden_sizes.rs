#[test]
fn mega_is_1048576() {
    assert_eq!(t::parse_size("1m"), Some(1_048_576));
}

#[test]
fn plain_number() {
    assert_eq!(t::parse_size("42"), Some(42));
}

#[test]
fn junk_is_none() {
    assert_eq!(t::parse_size("x1"), None);
}
