#[test]
fn normalize_collapses_whitespace() {
    assert_eq!(wcore::normalize("A\t B\n  C"), "a b c");
}
