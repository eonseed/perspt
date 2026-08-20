#[test]
fn respond_normalizes() {
    assert_eq!(wapi::respond("  Hello   WORLD "), "ok:hello world");
}

#[test]
fn respond_empty() {
    assert_eq!(wapi::respond("   "), "ok:");
}
