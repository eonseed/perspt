use rust_recovery_cache::Cache;

#[test]
fn obeys_lru_and_updates() {
    let mut c = Cache::new(2);
    c.put("a".into(), 1);
    c.put("b".into(), 2);
    assert_eq!(c.get("a"), Some(1));
    c.put("c".into(), 3);
    assert_eq!(c.get("b"), None);
    assert_eq!(c.get("a"), Some(1));
    c.put("a".into(), 9);
    assert_eq!(c.get("a"), Some(9));
    let mut zero = Cache::new(0);
    zero.put("x".into(), 1);
    assert_eq!(zero.get("x"), None);
}
