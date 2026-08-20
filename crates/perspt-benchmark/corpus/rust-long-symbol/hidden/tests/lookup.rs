#[test]
fn finds_edges_and_marker() {
    assert_eq!(rust_long_symbol::lookup("entry-000000"), Some(0));
    assert_eq!(rust_long_symbol::lookup("release-channel"), Some(7319));
    assert_eq!(rust_long_symbol::lookup("entry-008999"), Some(8999));
    assert_eq!(rust_long_symbol::lookup("missing"), None);
}
