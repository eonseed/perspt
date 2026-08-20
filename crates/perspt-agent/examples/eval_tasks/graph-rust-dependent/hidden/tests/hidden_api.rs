#[test]
fn api_composes_helper() {
    assert_eq!(t::api(), "base-done");
}

#[test]
fn helper_is_base() {
    assert_eq!(t::a::helper(), "base");
}
